use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use super::protocol::{
    AutomationErrorCode, AutomationOperation, AutomationOutcome, AutomationRequest,
    AutomationResponse, MAX_FRAME_BYTES, MAX_SEARCH_LIMIT, MAX_TEXT_INPUT_BYTES,
};
use super::server::{send_request, AutomationTransportError};

const MAX_CLI_STDIN_BYTES: usize = MAX_TEXT_INPUT_BYTES;
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub const CLI_USAGE: &str = "\
Usage:
  clipriva automation status [--json]
  clipriva automation search [--limit 1..50] [--json]       # query from stdin
  clipriva automation get --id <opaque-id> [--json]
  clipriva automation copy --id <opaque-id> [--plain-text] [--json]
  clipriva automation filter list [--json]
  clipriva automation filter apply --filter-id <opaque-id> --item-id <opaque-id> [--copy-output] [--json]
  clipriva automation filter apply --filter-id <opaque-id> --stdin [--copy-output] [--json]
  clipriva automation saved add --id <opaque-id> [--json]   # collection array from stdin
  clipriva automation saved remove --id <opaque-id> [--json]
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliParseError {
    Usage,
    InvalidValue,
    StdinTooLarge,
    InvalidStdin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum CliExitCode {
    Success = 0,
    Usage = 2,
    AppUnavailable = 3,
    Disabled = 4,
    CapabilityDenied = 5,
    NotFound = 6,
    InvalidInput = 7,
    OperationFailed = 8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliInvocation {
    pub request: AutomationRequest,
}

/// Parses arguments after the `automation` prefix.
///
/// Search queries, transform text and Collection names are read from stdin so
/// they cannot be exposed through argv, shell history, or process listings.
pub fn parse_cli_args(
    args: &[String],
    stdin: &mut impl Read,
) -> Result<CliInvocation, CliParseError> {
    let operation = match args.first().map(String::as_str) {
        Some("status") => parse_status(&args[1..])?,
        Some("search") => parse_search(&args[1..], stdin)?,
        Some("get") => parse_get(&args[1..])?,
        Some("copy") => parse_copy(&args[1..])?,
        Some("filter") => parse_filter(&args[1..], stdin)?,
        Some("saved") => parse_saved(&args[1..], stdin)?,
        _ => return Err(CliParseError::Usage),
    };
    let request = AutomationRequest::new(next_request_id(), operation);
    request
        .validate()
        .map_err(|_| CliParseError::InvalidValue)?;
    if serde_json::to_vec(&request)
        .map_err(|_| CliParseError::InvalidValue)?
        .len()
        > MAX_FRAME_BYTES
    {
        return Err(CliParseError::StdinTooLarge);
    }
    Ok(CliInvocation { request })
}

pub fn run_cli(
    args: &[String],
    stdin: &mut impl Read,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    socket_path: &Path,
) -> CliExitCode {
    run_cli_with_client(args, stdin, stdout, stderr, |request| {
        send_request(socket_path, request)
    })
}

pub fn run_cli_with_client<F>(
    args: &[String],
    stdin: &mut impl Read,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    client: F,
) -> CliExitCode
where
    F: FnOnce(&AutomationRequest) -> Result<AutomationResponse, AutomationTransportError>,
{
    let invocation = match parse_cli_args(args, stdin) {
        Ok(invocation) => invocation,
        Err(error) => {
            let _ = writeln!(
                stderr,
                "clipriva automation: {}",
                cli_parse_error_label(error)
            );
            let _ = write!(stderr, "{CLI_USAGE}");
            return CliExitCode::Usage;
        }
    };
    let response = match client(&invocation.request) {
        Ok(response) => response,
        Err(AutomationTransportError::ConnectFailed) => {
            let _ = writeln!(stderr, "clipriva automation: appUnavailable");
            return CliExitCode::AppUnavailable;
        }
        Err(AutomationTransportError::FrameTooLarge) => {
            let _ = writeln!(stderr, "clipriva automation: requestTooLarge");
            return CliExitCode::InvalidInput;
        }
        Err(_) => {
            let _ = writeln!(stderr, "clipriva automation: transportFailed");
            return CliExitCode::OperationFailed;
        }
    };

    if serde_json::to_writer(&mut *stdout, &response).is_err() || writeln!(stdout).is_err() {
        let _ = writeln!(stderr, "clipriva automation: outputFailed");
        return CliExitCode::OperationFailed;
    }
    exit_code_for_response(&response)
}

fn parse_status(args: &[String]) -> Result<AutomationOperation, CliParseError> {
    accept_json_only(args)?;
    Ok(AutomationOperation::Status {})
}

fn parse_search(
    args: &[String],
    stdin: &mut impl Read,
) -> Result<AutomationOperation, CliParseError> {
    let mut limit = 10;
    let mut saw_limit = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => index += 1,
            "--limit" if !saw_limit => {
                let value = args.get(index + 1).ok_or(CliParseError::Usage)?;
                limit = value
                    .parse::<u16>()
                    .map_err(|_| CliParseError::InvalidValue)?;
                if !(1..=MAX_SEARCH_LIMIT).contains(&limit) {
                    return Err(CliParseError::InvalidValue);
                }
                saw_limit = true;
                index += 2;
            }
            _ => return Err(CliParseError::Usage),
        }
    }
    let query = read_stdin_string(stdin)?;
    Ok(AutomationOperation::Search { query, limit })
}

fn parse_get(args: &[String]) -> Result<AutomationOperation, CliParseError> {
    let id = parse_required_id(args, false)?;
    Ok(AutomationOperation::GetText { item_id: id })
}

fn parse_copy(args: &[String]) -> Result<AutomationOperation, CliParseError> {
    let mut id = None;
    let mut plain_text = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--id" if id.is_none() => {
                id = Some(args.get(index + 1).ok_or(CliParseError::Usage)?.clone());
                index += 2;
            }
            "--plain-text" if !plain_text => {
                plain_text = true;
                index += 1;
            }
            "--json" => index += 1,
            _ => return Err(CliParseError::Usage),
        }
    }
    Ok(AutomationOperation::CopyItem {
        item_id: id.ok_or(CliParseError::Usage)?,
        plain_text,
    })
}

fn parse_filter(
    args: &[String],
    stdin: &mut impl Read,
) -> Result<AutomationOperation, CliParseError> {
    match args.first().map(String::as_str) {
        Some("list") => {
            accept_json_only(&args[1..])?;
            Ok(AutomationOperation::ListFilters {})
        }
        Some("apply") => parse_filter_apply(&args[1..], stdin),
        _ => Err(CliParseError::Usage),
    }
}

fn parse_filter_apply(
    args: &[String],
    stdin: &mut impl Read,
) -> Result<AutomationOperation, CliParseError> {
    let mut filter_id = None;
    let mut item_id = None;
    let mut read_stdin = false;
    let mut copy_output = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--filter-id" if filter_id.is_none() => {
                filter_id = Some(args.get(index + 1).ok_or(CliParseError::Usage)?.clone());
                index += 2;
            }
            "--item-id" if item_id.is_none() => {
                item_id = Some(args.get(index + 1).ok_or(CliParseError::Usage)?.clone());
                index += 2;
            }
            "--stdin" if !read_stdin => {
                read_stdin = true;
                index += 1;
            }
            "--copy-output" if !copy_output => {
                copy_output = true;
                index += 1;
            }
            "--json" => index += 1,
            _ => return Err(CliParseError::Usage),
        }
    }
    let filter_id = filter_id.ok_or(CliParseError::Usage)?;
    match (item_id, read_stdin) {
        (Some(item_id), false) => Ok(AutomationOperation::ApplyFilterToItem {
            filter_id,
            item_id,
            copy_output,
        }),
        (None, true) => Ok(AutomationOperation::ApplyFilterToText {
            filter_id,
            input: read_stdin_string(stdin)?,
            copy_output,
        }),
        _ => Err(CliParseError::Usage),
    }
}

fn parse_saved(
    args: &[String],
    stdin: &mut impl Read,
) -> Result<AutomationOperation, CliParseError> {
    match args.first().map(String::as_str) {
        Some("add") => {
            let item_id = parse_required_id(&args[1..], false)?;
            let bytes = read_stdin_bytes(stdin)?;
            let collections = if bytes.is_empty() {
                Vec::new()
            } else {
                serde_json::from_slice::<Vec<String>>(&bytes)
                    .map_err(|_| CliParseError::InvalidStdin)?
            };
            Ok(AutomationOperation::SaveItem {
                item_id,
                collections,
            })
        }
        Some("remove") => Ok(AutomationOperation::RemoveSavedItem {
            item_id: parse_required_id(&args[1..], false)?,
        }),
        _ => Err(CliParseError::Usage),
    }
}

fn parse_required_id(args: &[String], allow_plain_text: bool) -> Result<String, CliParseError> {
    let mut id = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--id" if id.is_none() => {
                id = Some(args.get(index + 1).ok_or(CliParseError::Usage)?.clone());
                index += 2;
            }
            "--json" => index += 1,
            "--plain-text" if allow_plain_text => index += 1,
            _ => return Err(CliParseError::Usage),
        }
    }
    id.ok_or(CliParseError::Usage)
}

fn accept_json_only(args: &[String]) -> Result<(), CliParseError> {
    if args.is_empty() || args == ["--json"] {
        Ok(())
    } else {
        Err(CliParseError::Usage)
    }
}

fn read_stdin_string(stdin: &mut impl Read) -> Result<String, CliParseError> {
    String::from_utf8(read_stdin_bytes(stdin)?).map_err(|_| CliParseError::InvalidStdin)
}

fn read_stdin_bytes(stdin: &mut impl Read) -> Result<Vec<u8>, CliParseError> {
    let mut bytes = Vec::new();
    stdin
        .take((MAX_CLI_STDIN_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| CliParseError::InvalidStdin)?;
    if bytes.len() > MAX_CLI_STDIN_BYTES {
        return Err(CliParseError::StdinTooLarge);
    }
    Ok(bytes)
}

fn next_request_id() -> String {
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("cli-{}-{sequence}", std::process::id())
}

fn cli_parse_error_label(error: CliParseError) -> &'static str {
    match error {
        CliParseError::Usage => "invalidArguments",
        CliParseError::InvalidValue => "invalidValue",
        CliParseError::StdinTooLarge => "stdinTooLarge",
        CliParseError::InvalidStdin => "invalidStdin",
    }
}

fn exit_code_for_response(response: &AutomationResponse) -> CliExitCode {
    let AutomationOutcome::Failure { error } = &response.outcome else {
        return CliExitCode::Success;
    };
    match error.code {
        AutomationErrorCode::Disabled => CliExitCode::Disabled,
        AutomationErrorCode::CapabilityDenied => CliExitCode::CapabilityDenied,
        AutomationErrorCode::NotFound => CliExitCode::NotFound,
        AutomationErrorCode::InvalidJson
        | AutomationErrorCode::InvalidRequest
        | AutomationErrorCode::InvalidInput
        | AutomationErrorCode::ProtocolVersionMismatch
        | AutomationErrorCode::FrameTooLarge => CliExitCode::InvalidInput,
        AutomationErrorCode::AppUnavailable => CliExitCode::AppUnavailable,
        AutomationErrorCode::OutputTooLarge
        | AutomationErrorCode::Busy
        | AutomationErrorCode::ExecutionFailed
        | AutomationErrorCode::TransportFailed => CliExitCode::OperationFailed,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::automation::protocol::{
        AutomationFailure, AutomationOutput, AUTOMATION_PROTOCOL_VERSION,
    };

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn search_query_is_accepted_from_stdin_and_rejected_from_argv() {
        let private_query = "synthetic quarterly phrase";
        let invocation = parse_cli_args(
            &strings(&["search", "--limit", "7"]),
            &mut Cursor::new(private_query),
        )
        .unwrap();
        assert!(matches!(
            invocation.request.operation,
            AutomationOperation::Search { ref query, limit: 7 } if query == private_query
        ));

        let error = parse_cli_args(
            &strings(&["search", "--query", private_query]),
            &mut Cursor::new(Vec::<u8>::new()),
        )
        .unwrap_err();
        assert_eq!(error, CliParseError::Usage);
    }

    #[test]
    fn transform_text_and_collection_names_are_stdin_only() {
        let invocation = parse_cli_args(
            &strings(&["filter", "apply", "--filter-id", "filter-1", "--stdin"]),
            &mut Cursor::new(" private\ntext "),
        )
        .unwrap();
        assert!(matches!(
            invocation.request.operation,
            AutomationOperation::ApplyFilterToText { ref input, .. } if input == " private\ntext "
        ));

        let invocation = parse_cli_args(
            &strings(&["saved", "add", "--id", "item-1"]),
            &mut Cursor::new(r#"["Synthetic Work"]"#),
        )
        .unwrap();
        assert!(matches!(
            invocation.request.operation,
            AutomationOperation::SaveItem { ref collections, .. }
                if collections == &["Synthetic Work".to_owned()]
        ));

        assert!(parse_cli_args(
            &strings(&["saved", "add", "--id", "item-1", "--collection", "Private",]),
            &mut Cursor::new(Vec::<u8>::new()),
        )
        .is_err());
    }

    #[test]
    fn rejects_path_sql_shell_and_mcp_cli_surfaces() {
        for args in [
            strings(&["path", "/tmp/value"]),
            strings(&["sql", "select"]),
            strings(&["shell", "echo"]),
            strings(&["mcp", "serve"]),
            strings(&["get", "--path", "/tmp/value"]),
        ] {
            assert!(parse_cli_args(&args, &mut Cursor::new(Vec::<u8>::new())).is_err());
        }
    }

    #[test]
    fn rejects_oversized_stdin_without_echoing_it() {
        let oversized = vec![b'x'; MAX_CLI_STDIN_BYTES + 1];
        assert_eq!(
            parse_cli_args(&strings(&["search"]), &mut Cursor::new(oversized)).unwrap_err(),
            CliParseError::StdinTooLarge
        );

        // Control characters expand during JSON encoding. Reject the encoded
        // request before connecting instead of misreporting a transport error.
        let expanding = vec![1_u8; 12_000];
        assert_eq!(
            parse_cli_args(
                &strings(&["filter", "apply", "--filter-id", "filter-1", "--stdin"]),
                &mut Cursor::new(expanding),
            )
            .unwrap_err(),
            CliParseError::StdinTooLarge
        );
    }

    #[test]
    fn run_cli_writes_only_bounded_json_and_static_errors() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit = run_cli_with_client(
            &strings(&["status"]),
            &mut Cursor::new(Vec::<u8>::new()),
            &mut stdout,
            &mut stderr,
            |request| {
                Ok(AutomationResponse::success(
                    request.request_id.clone(),
                    AutomationOutput::Status {
                        status: super::super::protocol::AutomationStatus {
                            enabled: false,
                            protocol_version: AUTOMATION_PROTOCOL_VERSION,
                            capabilities: Vec::new(),
                        },
                    },
                ))
            },
        );
        assert_eq!(exit, CliExitCode::Success);
        assert!(stderr.is_empty());
        assert!(serde_json::from_slice::<AutomationResponse>(&stdout).is_ok());

        let marker = "do-not-echo-private-marker";
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit = run_cli_with_client(
            &strings(&["search", "--query", marker]),
            &mut Cursor::new(Vec::<u8>::new()),
            &mut stdout,
            &mut stderr,
            |_request| unreachable!(),
        );
        assert_eq!(exit, CliExitCode::Usage);
        assert!(!String::from_utf8(stderr).unwrap().contains(marker));
    }

    #[test]
    fn maps_capability_denial_to_stable_exit_code() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit = run_cli_with_client(
            &strings(&["get", "--id", "item-1"]),
            &mut Cursor::new(Vec::<u8>::new()),
            &mut stdout,
            &mut stderr,
            |request| {
                Ok(AutomationResponse::failure(
                    Some(request.request_id.clone()),
                    AutomationFailure::capability_denied(
                        super::super::protocol::AutomationCapability::HistoryContent,
                    ),
                ))
            },
        );
        assert_eq!(exit, CliExitCode::CapabilityDenied);
        assert!(stderr.is_empty());
    }
}
