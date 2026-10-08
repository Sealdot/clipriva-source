use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::Serialize;

use super::protocol::{
    AutomationErrorCode, AutomationFailure, AutomationOperation, AutomationOutput,
    AutomationRequest, AutomationResponse, MAX_FRAME_BYTES,
};
use super::AutomationExecutor;

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);
const ACCEPT_RETRY_DELAY: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomationTransportError {
    SocketDirectoryMissing,
    SocketDirectoryNotPrivate,
    SocketAlreadyExists,
    BindFailed,
    ConnectFailed,
    ReadFailed,
    WriteFailed,
    FrameTooLarge,
    MissingLineTerminator,
    InvalidResponse,
    WorkerFailed,
}

#[derive(Debug, Clone, Copy)]
struct SocketIdentity {
    device: u64,
    inode: u64,
}

/// A bounded, same-user Unix socket server.
///
/// `socket_path` is application configuration, not protocol input. Start fails
/// if the path already exists and never removes an unverified stale path.
pub struct AutomationServer {
    socket_path: PathBuf,
    socket_identity: SocketIdentity,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl AutomationServer {
    pub fn start(
        socket_path: PathBuf,
        executor: Arc<dyn AutomationExecutor>,
    ) -> Result<Self, AutomationTransportError> {
        validate_socket_parent(&socket_path)?;
        if fs::symlink_metadata(&socket_path).is_ok() {
            return Err(AutomationTransportError::SocketAlreadyExists);
        }

        let listener =
            UnixListener::bind(&socket_path).map_err(|_| AutomationTransportError::BindFailed)?;
        if let Err(_error) = fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600)) {
            let _ = remove_verified_socket(&socket_path, None);
            return Err(AutomationTransportError::BindFailed);
        }
        let metadata = match fs::symlink_metadata(&socket_path) {
            Ok(metadata) => metadata,
            Err(_) => {
                let _ = remove_verified_socket(&socket_path, None);
                return Err(AutomationTransportError::BindFailed);
            }
        };
        if !metadata.file_type().is_socket() || metadata.mode() & 0o077 != 0 {
            let _ = remove_verified_socket(&socket_path, None);
            return Err(AutomationTransportError::BindFailed);
        }
        let socket_identity = SocketIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        if listener.set_nonblocking(true).is_err() {
            let _ = remove_verified_socket(&socket_path, Some(socket_identity));
            return Err(AutomationTransportError::BindFailed);
        }

        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker_path = socket_path.clone();
        let worker = thread::Builder::new()
            .name("clipriva-automation-v1".to_owned())
            .spawn(move || {
                serve(listener, executor, &worker_stop);
                let _ = remove_verified_socket(&worker_path, Some(socket_identity));
            })
            .map_err(|_| {
                let _ = remove_verified_socket(&socket_path, Some(socket_identity));
                AutomationTransportError::WorkerFailed
            })?;

        Ok(Self {
            socket_path,
            socket_identity,
            stop,
            worker: Some(worker),
        })
    }

    pub fn shutdown(&mut self) -> Result<(), AutomationTransportError> {
        self.stop.store(true, Ordering::Release);
        // Wake a nonblocking accept loop immediately. The worker observes stop
        // before processing the wake connection.
        let _ = UnixStream::connect(&self.socket_path);
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| AutomationTransportError::WorkerFailed)?;
        }
        remove_verified_socket(&self.socket_path, Some(self.socket_identity))
            .map_err(|_| AutomationTransportError::WorkerFailed)
    }
}

impl Drop for AutomationServer {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn validate_socket_parent(socket_path: &Path) -> Result<(), AutomationTransportError> {
    let parent = socket_path
        .parent()
        .ok_or(AutomationTransportError::SocketDirectoryMissing)?;
    let metadata = fs::symlink_metadata(parent)
        .map_err(|_| AutomationTransportError::SocketDirectoryMissing)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || metadata.mode() & 0o077 != 0 {
        return Err(AutomationTransportError::SocketDirectoryNotPrivate);
    }
    Ok(())
}

fn remove_verified_socket(path: &Path, expected: Option<SocketIdentity>) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_socket() {
        return Ok(());
    }
    if let Some(expected) = expected {
        if metadata.dev() != expected.device || metadata.ino() != expected.inode {
            return Ok(());
        }
    }
    fs::remove_file(path)
}

fn serve(listener: UnixListener, executor: Arc<dyn AutomationExecutor>, stop: &AtomicBool) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _address)) => {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                serve_connection(stream, executor.as_ref());
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_RETRY_DELAY);
            }
            Err(_) => break,
        }
    }
}

fn serve_connection(mut stream: UnixStream, executor: &dyn AutomationExecutor) {
    // The listener polls in nonblocking mode; accepted sockets can inherit
    // that mode on macOS. Read the framed request with the bounded timeout.
    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let _ = stream.set_read_timeout(Some(CONNECTION_TIMEOUT));
    let _ = stream.set_write_timeout(Some(CONNECTION_TIMEOUT));
    let response = {
        let mut reader = BufReader::new(&stream);
        match read_json_line(&mut reader) {
            Ok(frame) => process_frame(&frame, executor),
            Err(error) => frame_error_response(error),
        }
    };
    let _ = write_response(&mut stream, response);
}

fn frame_error_response(error: FrameReadError) -> AutomationResponse {
    let code = match error {
        FrameReadError::TooLarge => AutomationErrorCode::FrameTooLarge,
        FrameReadError::Empty | FrameReadError::MissingTerminator | FrameReadError::Io => {
            AutomationErrorCode::InvalidRequest
        }
    };
    AutomationResponse::failure(None, AutomationFailure::new(code))
}

fn process_frame(frame: &[u8], executor: &dyn AutomationExecutor) -> AutomationResponse {
    let request = match serde_json::from_slice::<AutomationRequest>(frame) {
        Ok(request) => request,
        Err(_) => {
            return AutomationResponse::failure(
                None,
                AutomationFailure::new(AutomationErrorCode::InvalidJson),
            )
        }
    };
    let request_id = request.request_id.clone();
    if let Err(error) = request.validate() {
        return AutomationResponse::failure(Some(request_id), error);
    }

    if matches!(request.operation, AutomationOperation::Status {}) {
        return AutomationResponse::success(
            request_id,
            AutomationOutput::Status {
                status: executor.status(),
            },
        );
    }

    if !executor.status().enabled {
        return AutomationResponse::failure(
            Some(request_id),
            AutomationFailure::new(AutomationErrorCode::Disabled),
        );
    }
    for capability in request.operation.required_capabilities() {
        if !executor.capability_enabled(capability) {
            return AutomationResponse::failure(
                Some(request_id),
                AutomationFailure::capability_denied(capability),
            );
        }
    }

    match executor.execute(&request.operation) {
        Ok(output) => AutomationResponse::success(request_id, output),
        Err(error) => AutomationResponse::failure(Some(request_id), error),
    }
}

fn write_response(
    writer: &mut impl Write,
    response: AutomationResponse,
) -> Result<(), AutomationTransportError> {
    match write_json_line(writer, &response) {
        Ok(()) => Ok(()),
        Err(AutomationTransportError::FrameTooLarge) => {
            let bounded = AutomationResponse::failure(
                response.request_id,
                AutomationFailure::new(AutomationErrorCode::OutputTooLarge),
            );
            write_json_line(writer, &bounded)
        }
        Err(error) => Err(error),
    }
}

pub fn send_request(
    socket_path: &Path,
    request: &AutomationRequest,
) -> Result<AutomationResponse, AutomationTransportError> {
    let mut stream =
        UnixStream::connect(socket_path).map_err(|_| AutomationTransportError::ConnectFailed)?;
    stream
        .set_read_timeout(Some(CONNECTION_TIMEOUT))
        .map_err(|_| AutomationTransportError::ConnectFailed)?;
    stream
        .set_write_timeout(Some(CONNECTION_TIMEOUT))
        .map_err(|_| AutomationTransportError::ConnectFailed)?;
    write_json_line(&mut stream, request)?;
    let mut reader = BufReader::new(stream);
    let frame = read_json_line(&mut reader).map_err(|error| match error {
        FrameReadError::TooLarge => AutomationTransportError::FrameTooLarge,
        FrameReadError::MissingTerminator => AutomationTransportError::MissingLineTerminator,
        FrameReadError::Empty | FrameReadError::Io => AutomationTransportError::ReadFailed,
    })?;
    serde_json::from_slice(&frame).map_err(|_| AutomationTransportError::InvalidResponse)
}

fn write_json_line(
    writer: &mut impl Write,
    value: &impl Serialize,
) -> Result<(), AutomationTransportError> {
    let encoded = serde_json::to_vec(value).map_err(|_| AutomationTransportError::WriteFailed)?;
    if encoded.len() > MAX_FRAME_BYTES {
        return Err(AutomationTransportError::FrameTooLarge);
    }
    writer
        .write_all(&encoded)
        .and_then(|_| writer.write_all(b"\n"))
        .and_then(|_| writer.flush())
        .map_err(|_| AutomationTransportError::WriteFailed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameReadError {
    Empty,
    TooLarge,
    MissingTerminator,
    Io,
}

fn read_json_line(reader: &mut impl BufRead) -> Result<Vec<u8>, FrameReadError> {
    let mut frame = Vec::new();
    loop {
        let available = reader.fill_buf().map_err(|_| FrameReadError::Io)?;
        if available.is_empty() {
            return if frame.is_empty() {
                Err(FrameReadError::Empty)
            } else {
                Err(FrameReadError::MissingTerminator)
            };
        }
        if let Some(newline) = available.iter().position(|byte| *byte == b'\n') {
            if frame.len().saturating_add(newline) > MAX_FRAME_BYTES {
                return Err(FrameReadError::TooLarge);
            }
            frame.extend_from_slice(&available[..newline]);
            reader.consume(newline + 1);
            return if frame.is_empty() {
                Err(FrameReadError::Empty)
            } else {
                Ok(frame)
            };
        }

        let available_len = available.len();
        if frame.len().saturating_add(available_len) > MAX_FRAME_BYTES {
            return Err(FrameReadError::TooLarge);
        }
        frame.extend_from_slice(available);
        reader.consume(available_len);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::io::{BufReader, Cursor, Read};
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, AtomicUsize};
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::automation::protocol::{
        AutomationCapability, AutomationItemSummary, AutomationOutcome, AutomationStatus,
        AUTOMATION_PROTOCOL_VERSION,
    };

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    struct TempSocket {
        directory: PathBuf,
        socket: PathBuf,
    }

    impl TempSocket {
        fn new() -> Self {
            let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "clipriva-automation-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&directory).unwrap();
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
            let socket = directory.join("automation-v1.sock");
            Self { directory, socket }
        }
    }

    impl Drop for TempSocket {
        fn drop(&mut self) {
            if let Ok(metadata) = fs::symlink_metadata(&self.socket) {
                if metadata.file_type().is_socket() {
                    let _ = fs::remove_file(&self.socket);
                }
            }
            let _ = fs::remove_dir(&self.directory);
        }
    }

    struct TestExecutor {
        enabled: AtomicBool,
        capabilities: Mutex<HashSet<AutomationCapability>>,
        executions: AtomicUsize,
        oversized_output: AtomicBool,
    }

    impl TestExecutor {
        fn new(enabled: bool, capabilities: &[AutomationCapability]) -> Self {
            Self {
                enabled: AtomicBool::new(enabled),
                capabilities: Mutex::new(capabilities.iter().copied().collect()),
                executions: AtomicUsize::new(0),
                oversized_output: AtomicBool::new(false),
            }
        }
    }

    impl AutomationExecutor for TestExecutor {
        fn status(&self) -> AutomationStatus {
            AutomationStatus {
                enabled: self.enabled.load(Ordering::Acquire),
                protocol_version: AUTOMATION_PROTOCOL_VERSION,
                capabilities: self.capabilities.lock().unwrap().iter().copied().collect(),
            }
        }

        fn capability_enabled(&self, capability: AutomationCapability) -> bool {
            self.capabilities.lock().unwrap().contains(&capability)
        }

        fn execute(
            &self,
            operation: &AutomationOperation,
        ) -> Result<AutomationOutput, AutomationFailure> {
            self.executions.fetch_add(1, Ordering::AcqRel);
            match operation {
                AutomationOperation::Search { .. } => Ok(AutomationOutput::SearchResults {
                    items: vec![AutomationItemSummary {
                        id: "item-1".to_owned(),
                        kind: "text".to_owned(),
                        created_at: "2026-08-23T00:00:00Z".to_owned(),
                        is_saved: false,
                    }],
                }),
                AutomationOperation::GetText { .. }
                    if self.oversized_output.load(Ordering::Acquire) =>
                {
                    Ok(AutomationOutput::Text {
                        content: "x".repeat(MAX_FRAME_BYTES),
                    })
                }
                AutomationOperation::GetText { .. } => Ok(AutomationOutput::Text {
                    content: "synthetic text".to_owned(),
                }),
                _ => Ok(AutomationOutput::Copied),
            }
        }
    }

    fn search_request() -> AutomationRequest {
        AutomationRequest::new(
            "request-1".to_owned(),
            AutomationOperation::Search {
                query: "synthetic".to_owned(),
                limit: 5,
            },
        )
    }

    #[test]
    fn frame_reader_requires_one_bounded_json_line() {
        assert_eq!(
            read_json_line(&mut BufReader::new(Cursor::new(b"{}\n"))).unwrap(),
            b"{}"
        );
        assert_eq!(
            read_json_line(&mut BufReader::new(Cursor::new(b"{}"))).unwrap_err(),
            FrameReadError::MissingTerminator
        );
        let oversized = vec![b'x'; MAX_FRAME_BYTES + 1];
        assert_eq!(
            read_json_line(&mut BufReader::new(Cursor::new(oversized))).unwrap_err(),
            FrameReadError::TooLarge
        );
    }

    #[test]
    fn disabled_and_missing_capability_requests_fail_before_execution() {
        let disabled = TestExecutor::new(false, &[AutomationCapability::HistoryMetadata]);
        let response = process_frame(&serde_json::to_vec(&search_request()).unwrap(), &disabled);
        assert!(matches!(
            response.outcome,
            AutomationOutcome::Failure {
                error: AutomationFailure {
                    code: AutomationErrorCode::Disabled,
                    ..
                }
            }
        ));
        assert_eq!(disabled.executions.load(Ordering::Acquire), 0);

        let denied = TestExecutor::new(true, &[]);
        let response = process_frame(&serde_json::to_vec(&search_request()).unwrap(), &denied);
        assert!(matches!(
            response.outcome,
            AutomationOutcome::Failure {
                error: AutomationFailure {
                    code: AutomationErrorCode::CapabilityDenied,
                    capability: Some(AutomationCapability::HistoryMetadata),
                }
            }
        ));
        assert_eq!(denied.executions.load(Ordering::Acquire), 0);
    }

    #[test]
    fn status_remains_content_free_and_available_while_disabled() {
        let executor = TestExecutor::new(false, &[]);
        let request = AutomationRequest::new("status-1".to_owned(), AutomationOperation::Status {});
        let response = process_frame(&serde_json::to_vec(&request).unwrap(), &executor);
        assert!(matches!(
            response.outcome,
            AutomationOutcome::Success {
                output: AutomationOutput::Status {
                    status: AutomationStatus { enabled: false, .. }
                }
            }
        ));
        assert_eq!(executor.executions.load(Ordering::Acquire), 0);
    }

    #[test]
    fn server_client_round_trip_enforces_private_socket_and_cleans_up() {
        let fixture = TempSocket::new();
        let executor = Arc::new(TestExecutor::new(
            true,
            &[AutomationCapability::HistoryMetadata],
        ));
        let mut server = AutomationServer::start(fixture.socket.clone(), executor).unwrap();
        let mode = fs::symlink_metadata(&fixture.socket)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);

        let response = send_request(&fixture.socket, &search_request()).unwrap();
        assert!(matches!(
            response.outcome,
            AutomationOutcome::Success {
                output: AutomationOutput::SearchResults { .. }
            }
        ));
        server.shutdown().unwrap();
        assert!(!fixture.socket.exists());
    }

    #[test]
    fn oversized_request_and_response_fail_with_bounded_codes() {
        let fixture = TempSocket::new();
        let executor = Arc::new(TestExecutor::new(
            true,
            &[
                AutomationCapability::HistoryMetadata,
                AutomationCapability::HistoryContent,
            ],
        ));
        executor.oversized_output.store(true, Ordering::Release);
        let mut server = AutomationServer::start(fixture.socket.clone(), executor).unwrap();

        let response = frame_error_response(FrameReadError::TooLarge);
        assert!(matches!(
            response.outcome,
            AutomationOutcome::Failure {
                error: AutomationFailure {
                    code: AutomationErrorCode::FrameTooLarge,
                    ..
                }
            }
        ));

        let get = AutomationRequest::new(
            "get-1".to_owned(),
            AutomationOperation::GetText {
                item_id: "item-1".to_owned(),
            },
        );
        let response = send_request(&fixture.socket, &get).unwrap();
        assert!(matches!(
            response.outcome,
            AutomationOutcome::Failure {
                error: AutomationFailure {
                    code: AutomationErrorCode::OutputTooLarge,
                    ..
                }
            }
        ));
        server.shutdown().unwrap();
    }

    #[test]
    fn existing_or_non_private_socket_targets_fail_without_deleting_them() {
        let fixture = TempSocket::new();
        fs::write(&fixture.socket, b"synthetic marker").unwrap();
        let result = AutomationServer::start(
            fixture.socket.clone(),
            Arc::new(TestExecutor::new(false, &[])),
        );
        assert!(matches!(
            result,
            Err(AutomationTransportError::SocketAlreadyExists)
        ));
        let mut marker = String::new();
        fs::File::open(&fixture.socket)
            .unwrap()
            .read_to_string(&mut marker)
            .unwrap();
        assert_eq!(marker, "synthetic marker");
        fs::remove_file(&fixture.socket).unwrap();

        fs::set_permissions(&fixture.directory, fs::Permissions::from_mode(0o755)).unwrap();
        let result = AutomationServer::start(
            fixture.socket.clone(),
            Arc::new(TestExecutor::new(false, &[])),
        );
        assert!(matches!(
            result,
            Err(AutomationTransportError::SocketDirectoryNotPrivate)
        ));
    }
}
