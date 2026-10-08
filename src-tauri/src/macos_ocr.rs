#[cfg(target_os = "macos")]
pub fn recognize_text(image_bytes: &[u8]) -> Result<String, &'static str> {
    use objc2::rc::autoreleasepool;
    use objc2::runtime::AnyObject;
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSData, NSDictionary};
    use objc2_vision::{
        VNImageOption, VNImageRequestHandler, VNRecognizeTextRequest, VNRequest,
        VNRequestTextRecognitionLevel,
    };

    if image_bytes.is_empty() {
        return Err("Local image text extraction could not read the image.");
    }

    autoreleasepool(|_| {
        let data = NSData::with_bytes(image_bytes);
        let options = NSDictionary::<VNImageOption, AnyObject>::new();
        let handler = VNImageRequestHandler::initWithData_options(
            VNImageRequestHandler::alloc(),
            &data,
            &options,
        );
        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(true);

        let base_request: objc2::rc::Retained<VNRequest> =
            request.clone().into_super().into_super();
        let requests = NSArray::from_retained_slice(&[base_request]);
        handler
            .performRequests_error(&requests)
            .map_err(|_| "Local image text extraction failed.")?;

        let Some(observations) = request.results() else {
            return Ok(String::new());
        };
        let mut lines = Vec::with_capacity(observations.len());
        for observation in observations.iter() {
            let candidates = observation.topCandidates(1);
            if let Some(candidate) = candidates.firstObject() {
                let line = candidate.string().to_string();
                let line = line.trim();
                if !line.is_empty() {
                    lines.push(line.to_owned());
                }
            }
        }
        Ok(lines.join("\n"))
    })
}

#[cfg(not(target_os = "macos"))]
pub fn recognize_text(_image_bytes: &[u8]) -> Result<String, &'static str> {
    Err("Local image text extraction is supported only on macOS.")
}

#[cfg(test)]
mod tests {
    use super::OcrJobCoordinator;
    use crate::clipboard::image_capture::{encode_rgba_png, CapturedImage};
    use crate::db::Database;
    use crate::media::ImageDimensions;

    #[test]
    fn admits_one_worker_and_never_commits_a_cancelled_generation() {
        let jobs = OcrJobCoordinator::default();
        let first = jobs.begin("ocr-request-00000001").unwrap();
        assert!(jobs.begin("ocr-request-00000002").is_err());
        assert!(jobs.cancel("ocr-request-00000001"));

        let mut persisted = false;
        assert!(jobs
            .complete_if_current(&first, || persisted = true)
            .is_none());
        assert!(!persisted);

        let second = jobs.begin("ocr-request-00000002").unwrap();
        assert!(jobs.complete_if_current(&first, || ()).is_none());
        assert_eq!(
            jobs.complete_if_current(&second, || "stored"),
            Some("stored")
        );
    }

    #[test]
    fn invalid_or_stale_cancellation_cannot_cancel_a_new_generation() {
        let jobs = OcrJobCoordinator::default();
        assert!(jobs.begin("short").is_err());
        let first = jobs.begin("same-request-000001").unwrap();
        assert!(jobs.discard(&first));

        let second = jobs.begin("same-request-000001").unwrap();
        assert!(!jobs.cancel("different-request-01"));
        assert_eq!(jobs.complete_if_current(&second, || 7), Some(7));
    }

    #[test]
    fn cancellation_leaves_neither_extraction_nor_fts_match() {
        let database = Database::open(std::path::Path::new(":memory:")).unwrap();
        let item = database
            .capture_image(
                &CapturedImage {
                    png: encode_rgba_png(&[255, 255, 255, 255], 1, 1).unwrap(),
                    dimensions: ImageDimensions {
                        width: 1,
                        height: 1,
                    },
                },
                None,
            )
            .unwrap()
            .unwrap();
        let jobs = OcrJobCoordinator::default();
        let token = jobs.begin("ocr-persist-request-01").unwrap();
        assert!(jobs.cancel("ocr-persist-request-01"));

        let result = jobs.complete_if_current(&token, || {
            database.save_image_text_extraction(&item.id, "cancelled unique OCR phrase")
        });
        assert!(result.is_none());
        assert!(database.image_text_extraction(&item.id).unwrap().is_none());
        assert!(database
            .list("cancelled unique OCR phrase", false, 20, 0)
            .unwrap()
            .is_empty());
    }
}
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OcrJobToken {
    request_id: String,
    generation: u64,
}

#[derive(Debug)]
struct ActiveOcrJob {
    request_id: String,
    generation: u64,
    cancelled: bool,
}

#[derive(Debug, Default)]
struct OcrJobState {
    next_generation: u64,
    active: Option<ActiveOcrJob>,
}

/// Process-wide admission and commit gate for manual OCR. Vision itself is a
/// synchronous API, so cancellation invalidates the generation rather than
/// attempting to interrupt framework code mid-call. The active slot remains
/// occupied until that call returns, guaranteeing at most one worker.
#[derive(Debug, Default)]
pub(crate) struct OcrJobCoordinator {
    state: Mutex<OcrJobState>,
}

impl OcrJobCoordinator {
    pub(crate) fn begin(&self, request_id: &str) -> Result<OcrJobToken, &'static str> {
        if request_id.len() < 16
            || request_id.len() > 128
            || !request_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("Invalid local image text extraction request.");
        }
        let mut state = self.state.lock().expect("OCR job mutex poisoned");
        if state.active.is_some() {
            return Err("Another local image text extraction is already running.");
        }
        state.next_generation = state.next_generation.wrapping_add(1).max(1);
        let generation = state.next_generation;
        state.active = Some(ActiveOcrJob {
            request_id: request_id.to_owned(),
            generation,
            cancelled: false,
        });
        Ok(OcrJobToken {
            request_id: request_id.to_owned(),
            generation,
        })
    }

    pub(crate) fn cancel(&self, request_id: &str) -> bool {
        let mut state = self.state.lock().expect("OCR job mutex poisoned");
        let Some(active) = state.active.as_mut() else {
            return false;
        };
        if active.request_id != request_id || active.cancelled {
            return false;
        }
        active.cancelled = true;
        true
    }

    /// Runs the final persistence closure only while the exact token is still
    /// current. Holding the coordinator lock makes cancellation and commit a
    /// single linearized decision; a stale Vision result cannot write OCR or
    /// its FTS trigger rows.
    pub(crate) fn complete_if_current<T>(
        &self,
        token: &OcrJobToken,
        persist: impl FnOnce() -> T,
    ) -> Option<T> {
        let mut state = self.state.lock().expect("OCR job mutex poisoned");
        let current = state.active.as_ref().is_some_and(|active| {
            active.request_id == token.request_id
                && active.generation == token.generation
                && !active.cancelled
        });
        if !current {
            if state.active.as_ref().is_some_and(|active| {
                active.request_id == token.request_id && active.generation == token.generation
            }) {
                state.active = None;
            }
            return None;
        }
        let result = persist();
        state.active = None;
        Some(result)
    }

    /// Releases a worker that ended before commit and reports whether its
    /// generation remained current (so callers can distinguish error/cancel).
    pub(crate) fn discard(&self, token: &OcrJobToken) -> bool {
        let mut state = self.state.lock().expect("OCR job mutex poisoned");
        let current = state.active.as_ref().is_some_and(|active| {
            active.request_id == token.request_id
                && active.generation == token.generation
                && !active.cancelled
        });
        if state.active.as_ref().is_some_and(|active| {
            active.request_id == token.request_id && active.generation == token.generation
        }) {
            state.active = None;
        }
        current
    }
}

pub(crate) fn job_coordinator() -> &'static OcrJobCoordinator {
    static JOBS: OnceLock<OcrJobCoordinator> = OnceLock::new();
    JOBS.get_or_init(OcrJobCoordinator::default)
}

/// Keeps the global worker slot occupied even if the async IPC future is
/// dropped while synchronous Vision work is still running. Dropping a result
/// without committing releases only its exact generation.
pub(crate) struct OcrWorkerOutput {
    token: OcrJobToken,
    result: Option<Result<String, &'static str>>,
}

impl OcrWorkerOutput {
    pub(crate) fn run(token: OcrJobToken, image_bytes: &[u8]) -> Self {
        let mut output = Self {
            token,
            result: None,
        };
        output.result = Some(recognize_text(image_bytes));
        output
    }

    pub(crate) fn take_result(&mut self) -> Result<String, &'static str> {
        self.result
            .take()
            .expect("OCR worker result can only be consumed once")
    }
}

impl Drop for OcrWorkerOutput {
    fn drop(&mut self) {
        job_coordinator().discard(&self.token);
    }
}
