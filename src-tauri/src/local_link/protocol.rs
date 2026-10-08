//! Versioned, bounded Local Link wire protocol over Noise XX.
//!
//! Every application frame is authenticated and encrypted by Noise. There is
//! deliberately no plaintext codec or fallback path.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::models::LocalLinkTransferFailure;

use super::identity::LocalIdentity;
use super::state_machine::TerminalReceipt;
use super::{MAX_TEXT_PAYLOAD_BYTES, NOISE_PARAMS, PROTOCOL_VERSION, TRANSFER_TTL_SECONDS};

const MAX_HANDSHAKE_MESSAGE_BYTES: usize = 1_024;
const MAX_NOISE_CIPHERTEXT_BYTES: usize = 65_535;
const MAX_NOISE_PLAINTEXT_BYTES: usize = 60 * 1_024;
pub(crate) const PAYLOAD_CHUNK_BYTES: usize = 48 * 1_024;
const FRAME_JSON: u8 = 1;
const FRAME_PAYLOAD: u8 = 2;
const MAX_WIRE_ID_BYTES: usize = 64;
const MAX_DEVICE_ID_BYTES: usize = 128;
const MAX_DISPLAY_NAME_CHARS: usize = 80;
const PAIRING_TTL_SECONDS: i64 = 60;
const PUBLIC_KEY_FINGERPRINT_GROUPS: usize = 6;
const TRANSCRIPT_FINGERPRINT_GROUPS: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum WireMessage {
    SessionHello {
        version: u16,
        session_id: String,
        device_id: String,
        display_name: String,
    },
    PairingProposal(PairingProposal),
    PairingConfirm(PairingConfirm),
    PairingCommit(PairingCommit),
    Offer(TransferOffer),
    OfferDecision {
        transfer_id: String,
        decision: OfferDecision,
    },
    PayloadComplete {
        transfer_id: String,
    },
    /// A content-free acknowledgement that the receiver opened the pending
    /// request. It deliberately carries no body, preview, or decision.
    Viewed {
        transfer_id: String,
    },
    Receipt {
        transfer_id: String,
        receipt: TerminalReceipt,
    },
    ReceiptAck {
        transfer_id: String,
        receipt: TerminalReceipt,
    },
    StatusQuery {
        transfer_id: String,
    },
    StatusResponse {
        transfer_id: String,
        status: StatusResponseState,
    },
    Cancel {
        transfer_id: String,
    },
}

/// The frame expected immediately after an accepted offer. A sender may
/// cancel before any payload frame is sent, so the receiver must be able to
/// distinguish that control message without accepting a body first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PayloadOrCancel {
    Payload(Vec<u8>),
    Cancelled,
}

/// Content-free result of a receiver-side status lookup.
///
/// `Unknown` and `Pending` are intentionally distinct: an unknown transfer
/// may be retried only as a metadata query, while a pending transfer is known
/// to the authenticated receiver and is still waiting for its single terminal
/// decision. No variant can carry a body, preview, digest, or device identity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "state",
    content = "receipt",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum StatusResponseState {
    Unknown,
    Pending,
    Terminal(TerminalReceipt),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PairingProposal {
    pub version: u16,
    pub pairing_id: String,
    pub session_id: String,
    pub device_id: String,
    pub display_name: String,
    pub public_key_fingerprint: String,
    pub expires_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PairingConfirm {
    pub version: u16,
    pub pairing_id: String,
    pub session_id: String,
    pub transcript_fingerprint: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PairingCommit {
    pub version: u16,
    pub pairing_id: String,
    pub session_id: String,
    pub device_id: String,
    pub public_key_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TransferOffer {
    pub version: u16,
    pub session_id: String,
    pub transfer_id: String,
    pub nonce: String,
    pub created_at_ms: i64,
    pub ttl_seconds: u16,
    pub content_type: String,
    pub byte_size: u32,
    pub digest_sha256: String,
}

impl WireMessage {
    pub(crate) fn validate(&self, now_ms: i64) -> AppResult<()> {
        match self {
            Self::SessionHello {
                version,
                session_id,
                device_id,
                display_name,
            } => {
                validate_version(*version)?;
                validate_uuid(session_id, "session")?;
                validate_device_id(device_id)?;
                validate_display_name(display_name)
            }
            Self::PairingProposal(proposal) => proposal.validate(now_ms),
            Self::PairingConfirm(confirm) => confirm.validate(),
            Self::PairingCommit(commit) => commit.validate(),
            Self::Offer(offer) => offer.validate(now_ms).map_err(|_| {
                AppError::InvalidInput("Local Link transfer offer is invalid.".to_owned())
            }),
            Self::OfferDecision { transfer_id, .. }
            | Self::PayloadComplete { transfer_id }
            | Self::Viewed { transfer_id }
            | Self::StatusQuery { transfer_id }
            | Self::Cancel { transfer_id } => validate_uuid(transfer_id, "transfer"),
            Self::Receipt {
                transfer_id,
                receipt,
            }
            | Self::ReceiptAck {
                transfer_id,
                receipt,
            } => {
                validate_uuid(transfer_id, "transfer")?;
                validate_receipt(*receipt)
            }
            Self::StatusResponse {
                transfer_id,
                status,
            } => {
                validate_uuid(transfer_id, "transfer")?;
                if let StatusResponseState::Terminal(receipt) = status {
                    validate_receipt(*receipt)?;
                }
                Ok(())
            }
        }
    }
}

impl PairingProposal {
    fn validate(&self, now_ms: i64) -> AppResult<()> {
        validate_version(self.version)?;
        validate_uuid(&self.pairing_id, "pairing")?;
        validate_uuid(&self.session_id, "session")?;
        validate_device_id(&self.device_id)?;
        validate_display_name(&self.display_name)?;
        validate_fingerprint(&self.public_key_fingerprint, PUBLIC_KEY_FINGERPRINT_GROUPS)?;
        if self.expires_at_ms <= now_ms
            || self.expires_at_ms > now_ms.saturating_add(PAIRING_TTL_SECONDS * 1_000)
        {
            return Err(AppError::InvalidInput(
                "Local Link pairing proposal is expired or has an invalid deadline.".to_owned(),
            ));
        }
        Ok(())
    }
}

impl PairingConfirm {
    fn validate(&self) -> AppResult<()> {
        validate_version(self.version)?;
        validate_uuid(&self.pairing_id, "pairing")?;
        validate_uuid(&self.session_id, "session")?;
        validate_fingerprint(&self.transcript_fingerprint, TRANSCRIPT_FINGERPRINT_GROUPS)
    }
}

impl PairingCommit {
    fn validate(&self) -> AppResult<()> {
        validate_version(self.version)?;
        validate_uuid(&self.pairing_id, "pairing")?;
        validate_uuid(&self.session_id, "session")?;
        validate_device_id(&self.device_id)?;
        validate_fingerprint(&self.public_key_fingerprint, PUBLIC_KEY_FINGERPRINT_GROUPS)
    }
}

impl TransferOffer {
    pub(crate) fn new(session_id: String, transfer_id: String, content: &[u8]) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            session_id,
            transfer_id,
            nonce: uuid::Uuid::new_v4().to_string(),
            created_at_ms: Utc::now().timestamp_millis(),
            ttl_seconds: TRANSFER_TTL_SECONDS as u16,
            content_type: "text/plain; charset=utf-8".to_owned(),
            byte_size: u32::try_from(content.len()).unwrap_or(u32::MAX),
            digest_sha256: digest_hex(content),
        }
    }

    pub(crate) fn validate(&self, now_ms: i64) -> Result<(), LocalLinkTransferFailure> {
        if self.version != PROTOCOL_VERSION {
            return Err(LocalLinkTransferFailure::VersionMismatch);
        }
        if uuid::Uuid::parse_str(&self.session_id).is_err()
            || uuid::Uuid::parse_str(&self.transfer_id).is_err()
            || uuid::Uuid::parse_str(&self.nonce).is_err()
        {
            return Err(LocalLinkTransferFailure::ProtocolViolation);
        }
        if self.ttl_seconds == 0 || i64::from(self.ttl_seconds) > TRANSFER_TTL_SECONDS {
            return Err(LocalLinkTransferFailure::ProtocolViolation);
        }
        let age_ms = now_ms.saturating_sub(self.created_at_ms);
        if self.created_at_ms > now_ms.saturating_add(30_000)
            || age_ms > i64::from(self.ttl_seconds) * 1_000
        {
            return Err(LocalLinkTransferFailure::PayloadUnavailable);
        }
        if self.content_type != "text/plain; charset=utf-8"
            || self.byte_size == 0
            || self.byte_size as usize > MAX_TEXT_PAYLOAD_BYTES
            || self.digest_sha256.len() != 64
            || !self
                .digest_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(LocalLinkTransferFailure::UnsupportedItem);
        }
        Ok(())
    }

    pub(crate) fn verify_payload(&self, payload: &[u8]) -> Result<(), LocalLinkTransferFailure> {
        if payload.len() != self.byte_size as usize
            || digest_hex(payload) != self.digest_sha256.to_ascii_lowercase()
            || std::str::from_utf8(payload).is_err()
        {
            return Err(LocalLinkTransferFailure::ProtocolViolation);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "result",
    content = "failure",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum OfferDecision {
    Ready,
    Rejected(LocalLinkTransferFailure),
}

/// A Noise transport bound to one concrete byte stream. This type has no
/// `Debug` implementation because its transport state contains session keys.
pub(crate) struct SecureChannel<S> {
    stream: S,
    transport: snow::TransportState,
    remote_static: Vec<u8>,
    handshake_hash: Vec<u8>,
}

impl<S: Read + Write> SecureChannel<S> {
    pub(crate) fn initiator(mut stream: S, identity: &LocalIdentity) -> AppResult<Self> {
        let mut handshake = noise_builder(identity)?
            .build_initiator()
            .map_err(noise_error)?;
        let mut buffer = [0_u8; MAX_HANDSHAKE_MESSAGE_BYTES];

        let written = handshake
            .write_message(&[], &mut buffer)
            .map_err(noise_error)?;
        write_bounded(&mut stream, &buffer[..written], MAX_HANDSHAKE_MESSAGE_BYTES)?;

        let message = read_bounded(&mut stream, MAX_HANDSHAKE_MESSAGE_BYTES)?;
        handshake
            .read_message(&message, &mut buffer)
            .map_err(noise_error)?;

        let written = handshake
            .write_message(&[], &mut buffer)
            .map_err(noise_error)?;
        write_bounded(&mut stream, &buffer[..written], MAX_HANDSHAKE_MESSAGE_BYTES)?;
        Self::finish(stream, handshake)
    }

    pub(crate) fn responder(mut stream: S, identity: &LocalIdentity) -> AppResult<Self> {
        let mut handshake = noise_builder(identity)?
            .build_responder()
            .map_err(noise_error)?;
        let mut buffer = [0_u8; MAX_HANDSHAKE_MESSAGE_BYTES];

        let message = read_bounded(&mut stream, MAX_HANDSHAKE_MESSAGE_BYTES)?;
        handshake
            .read_message(&message, &mut buffer)
            .map_err(noise_error)?;

        let written = handshake
            .write_message(&[], &mut buffer)
            .map_err(noise_error)?;
        write_bounded(&mut stream, &buffer[..written], MAX_HANDSHAKE_MESSAGE_BYTES)?;

        let message = read_bounded(&mut stream, MAX_HANDSHAKE_MESSAGE_BYTES)?;
        handshake
            .read_message(&message, &mut buffer)
            .map_err(noise_error)?;
        Self::finish(stream, handshake)
    }

    fn finish(stream: S, handshake: snow::HandshakeState) -> AppResult<Self> {
        if !handshake.is_handshake_finished() {
            return Err(AppError::InvalidInput(
                "Local Link authentication did not complete.".to_owned(),
            ));
        }
        let remote_static = handshake
            .get_remote_static()
            .ok_or_else(|| {
                AppError::InvalidInput("Local Link peer identity is missing.".to_owned())
            })?
            .to_vec();
        let handshake_hash = handshake.get_handshake_hash().to_vec();
        let transport = handshake.into_transport_mode().map_err(noise_error)?;
        Ok(Self {
            stream,
            transport,
            remote_static,
            handshake_hash,
        })
    }

    pub(crate) fn remote_static(&self) -> &[u8] {
        &self.remote_static
    }

    pub(crate) fn handshake_hash(&self) -> &[u8] {
        &self.handshake_hash
    }

    pub(crate) fn send_message(&mut self, message: &WireMessage) -> AppResult<()> {
        message.validate(Utc::now().timestamp_millis())?;
        let serialized = serde_json::to_vec(message)?;
        self.send_frame(FRAME_JSON, &serialized)
    }

    pub(crate) fn receive_message(&mut self) -> AppResult<WireMessage> {
        let (kind, payload) = self.receive_frame()?;
        if kind != FRAME_JSON {
            return Err(AppError::InvalidInput(
                "Local Link received an unexpected encrypted frame.".to_owned(),
            ));
        }
        let message: WireMessage = serde_json::from_slice(&payload)?;
        message.validate(Utc::now().timestamp_millis())?;
        Ok(message)
    }

    pub(crate) fn send_payload(&mut self, transfer_id: &str, payload: &[u8]) -> AppResult<()> {
        if payload.is_empty() || payload.len() > MAX_TEXT_PAYLOAD_BYTES {
            return Err(AppError::InvalidInput(
                "Local Link payload size is outside the text beta limit.".to_owned(),
            ));
        }
        let chunk_count = payload.len().div_ceil(PAYLOAD_CHUNK_BYTES);
        for (index, chunk) in payload.chunks(PAYLOAD_CHUNK_BYTES).enumerate() {
            let mut frame = Vec::with_capacity(1 + transfer_id.len() + 4 + chunk.len());
            let id_len = u8::try_from(transfer_id.len()).map_err(|_| {
                AppError::InvalidInput("Local Link transfer ID is too long.".to_owned())
            })?;
            frame.push(id_len);
            frame.extend_from_slice(transfer_id.as_bytes());
            frame.extend_from_slice(
                &u16::try_from(index)
                    .map_err(|_| AppError::InvalidInput("Too many payload chunks.".to_owned()))?
                    .to_be_bytes(),
            );
            frame.extend_from_slice(
                &u16::try_from(chunk_count)
                    .map_err(|_| AppError::InvalidInput("Too many payload chunks.".to_owned()))?
                    .to_be_bytes(),
            );
            frame.extend_from_slice(chunk);
            self.send_frame(FRAME_PAYLOAD, &frame)?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn receive_payload(
        &mut self,
        transfer_id: &str,
        expected_size: usize,
    ) -> AppResult<Vec<u8>> {
        match self.receive_payload_or_cancel(transfer_id, expected_size)? {
            PayloadOrCancel::Payload(payload) => Ok(payload),
            PayloadOrCancel::Cancelled => Err(AppError::InvalidInput(
                "Local Link payload was cancelled before it arrived.".to_owned(),
            )),
        }
    }

    /// Receive the body following an accepted offer, or an authenticated
    /// sender cancellation that arrives before the body. A cancellation after
    /// payload frames have begun also drops the assembled in-memory bytes and
    /// lets the service clear its pending body before a receiver decision.
    pub(crate) fn receive_payload_or_cancel(
        &mut self,
        transfer_id: &str,
        expected_size: usize,
    ) -> AppResult<PayloadOrCancel> {
        if expected_size == 0 || expected_size > MAX_TEXT_PAYLOAD_BYTES {
            return Err(AppError::InvalidInput(
                "Local Link payload size is outside the text beta limit.".to_owned(),
            ));
        }
        let expected_chunks = expected_size.div_ceil(PAYLOAD_CHUNK_BYTES);
        let mut output = Vec::with_capacity(expected_size);
        for expected_index in 0..expected_chunks {
            let (kind, frame) = self.receive_frame()?;
            if kind == FRAME_JSON {
                let message: WireMessage = serde_json::from_slice(&frame)?;
                message.validate(Utc::now().timestamp_millis())?;
                return match message {
                    WireMessage::Cancel {
                        transfer_id: cancelled_id,
                    } if cancelled_id == transfer_id => Ok(PayloadOrCancel::Cancelled),
                    _ => Err(AppError::InvalidInput(
                        "Local Link received an unexpected control message while receiving a payload."
                            .to_owned(),
                    )),
                };
            }
            if kind != FRAME_PAYLOAD || frame.len() < 5 {
                return Err(AppError::InvalidInput(
                    "Local Link payload framing is invalid.".to_owned(),
                ));
            }
            let id_len = usize::from(frame[0]);
            if frame.len() < 1 + id_len + 4 {
                return Err(AppError::InvalidInput(
                    "Local Link payload framing is invalid.".to_owned(),
                ));
            }
            let id = std::str::from_utf8(&frame[1..1 + id_len]).map_err(|_| {
                AppError::InvalidInput("Local Link transfer ID is invalid.".to_owned())
            })?;
            let index_offset = 1 + id_len;
            let index = u16::from_be_bytes([frame[index_offset], frame[index_offset + 1]]);
            let count = u16::from_be_bytes([frame[index_offset + 2], frame[index_offset + 3]]);
            if id != transfer_id
                || usize::from(index) != expected_index
                || usize::from(count) != expected_chunks
            {
                return Err(AppError::InvalidInput(
                    "Local Link payload chunk order is invalid.".to_owned(),
                ));
            }
            output.extend_from_slice(&frame[index_offset + 4..]);
            if output.len() > expected_size {
                return Err(AppError::InvalidInput(
                    "Local Link payload exceeded its declared size.".to_owned(),
                ));
            }
        }
        if output.len() != expected_size {
            return Err(AppError::InvalidInput(
                "Local Link payload did not match its declared size.".to_owned(),
            ));
        }
        Ok(PayloadOrCancel::Payload(output))
    }

    fn send_frame(&mut self, kind: u8, payload: &[u8]) -> AppResult<()> {
        if payload.len() + 1 > MAX_NOISE_PLAINTEXT_BYTES {
            return Err(AppError::InvalidInput(
                "Local Link encrypted frame is too large.".to_owned(),
            ));
        }
        let mut plaintext = Vec::with_capacity(payload.len() + 1);
        plaintext.push(kind);
        plaintext.extend_from_slice(payload);
        let mut ciphertext = vec![0_u8; plaintext.len() + 16];
        let written = self
            .transport
            .write_message(&plaintext, &mut ciphertext)
            .map_err(noise_error)?;
        ciphertext.truncate(written);
        write_bounded(&mut self.stream, &ciphertext, MAX_NOISE_CIPHERTEXT_BYTES)
    }

    fn receive_frame(&mut self) -> AppResult<(u8, Vec<u8>)> {
        let ciphertext = read_bounded(&mut self.stream, MAX_NOISE_CIPHERTEXT_BYTES)?;
        let mut plaintext = vec![0_u8; ciphertext.len()];
        let read = self
            .transport
            .read_message(&ciphertext, &mut plaintext)
            .map_err(noise_error)?;
        plaintext.truncate(read);
        if plaintext.is_empty() || plaintext.len() > MAX_NOISE_PLAINTEXT_BYTES {
            return Err(AppError::InvalidInput(
                "Local Link encrypted frame is invalid.".to_owned(),
            ));
        }
        let kind = plaintext.remove(0);
        Ok((kind, plaintext))
    }
}

impl SecureChannel<TcpStream> {
    /// Configure a bounded polling interval for a live transfer. Callers must
    /// use `poll_message` rather than a direct blocking receive while this is
    /// active so an incomplete TCP frame is never partially consumed on a
    /// timeout.
    pub(crate) fn set_poll_timeout(&self, timeout: Duration) -> AppResult<()> {
        self.stream.set_read_timeout(Some(timeout))?;
        Ok(())
    }

    /// Return a complete authenticated message if one is already buffered.
    /// `TcpStream::peek` never consumes bytes, so polling cannot split a
    /// length-prefixed Noise frame when the network delivers it in fragments.
    pub(crate) fn poll_message(&mut self) -> AppResult<Option<WireMessage>> {
        let mut buffered = vec![0_u8; MAX_NOISE_CIPHERTEXT_BYTES + 4];
        let available = match self.stream.peek(&mut buffered) {
            Ok(available) => available,
            Err(error) if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };
        if available == 0 {
            return Err(std::io::Error::from(ErrorKind::UnexpectedEof).into());
        }
        if available < 4 {
            return Ok(None);
        }
        let length =
            u32::from_be_bytes([buffered[0], buffered[1], buffered[2], buffered[3]]) as usize;
        if length == 0 || length > MAX_NOISE_CIPHERTEXT_BYTES {
            return Err(AppError::InvalidInput(
                "Local Link frame length is invalid.".to_owned(),
            ));
        }
        if available < length + 4 {
            return Ok(None);
        }
        self.receive_message().map(Some)
    }
}

pub(crate) fn pairing_code(handshake_hash: &[u8]) -> String {
    let digest = Sha256::digest(handshake_hash);
    let value = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) % 1_000_000;
    format!("{value:06}")
}

pub(crate) fn transcript_fingerprint(handshake_hash: &[u8]) -> String {
    Sha256::digest(handshake_hash)[..8]
        .chunks(2)
        .map(|chunk| format!("{:02X}{:02X}", chunk[0], chunk[1]))
        .collect::<Vec<_>>()
        .join("-")
}

pub(crate) fn digest_hex(payload: &[u8]) -> String {
    Sha256::digest(payload)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_version(version: u16) -> AppResult<()> {
    if version == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(AppError::InvalidInput(
            "Local Link protocol versions are incompatible.".to_owned(),
        ))
    }
}

fn validate_uuid(value: &str, label: &str) -> AppResult<()> {
    if value.is_empty() || value.len() > MAX_WIRE_ID_BYTES || uuid::Uuid::parse_str(value).is_err()
    {
        return Err(AppError::InvalidInput(format!(
            "Local Link {label} identifier is invalid."
        )));
    }
    Ok(())
}

fn validate_device_id(value: &str) -> AppResult<()> {
    if value.is_empty()
        || value.len() > MAX_DEVICE_ID_BYTES
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(AppError::InvalidInput(
            "Local Link device identifier is invalid.".to_owned(),
        ));
    }
    Ok(())
}

fn validate_display_name(value: &str) -> AppResult<()> {
    if value.trim().is_empty()
        || value.chars().count() > MAX_DISPLAY_NAME_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(AppError::InvalidInput(
            "Local Link device display name is invalid.".to_owned(),
        ));
    }
    Ok(())
}

fn validate_fingerprint(value: &str, groups: usize) -> AppResult<()> {
    let expected_length = groups * 4 + groups.saturating_sub(1);
    if value.len() != expected_length
        || value.split('-').count() != groups
        || value
            .split('-')
            .any(|group| group.len() != 4 || !group.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(AppError::InvalidInput(
            "Local Link fingerprint is invalid.".to_owned(),
        ));
    }
    Ok(())
}

fn validate_receipt(receipt: TerminalReceipt) -> AppResult<()> {
    if receipt.is_valid() {
        Ok(())
    } else {
        Err(AppError::InvalidInput(
            "Local Link receipt outcome and failure reason are inconsistent.".to_owned(),
        ))
    }
}

fn noise_builder(identity: &LocalIdentity) -> AppResult<snow::Builder<'_>> {
    let params = NOISE_PARAMS.parse().map_err(|_| {
        AppError::InvalidInput("Local Link Noise parameters are invalid.".to_owned())
    })?;
    snow::Builder::new(params)
        // v3 changes StatusResponse semantics. Binding the version into the
        // Noise transcript prevents an older peer from establishing a channel
        // and accidentally interpreting a control response under the legacy
        // optional-receipt schema.
        .prologue(b"ClipRiva Local Link v3")
        .and_then(|builder| builder.local_private_key(identity.private_key()))
        .map_err(noise_error)
}

fn write_bounded(stream: &mut impl Write, payload: &[u8], maximum: usize) -> AppResult<()> {
    if payload.is_empty() || payload.len() > maximum {
        return Err(AppError::InvalidInput(
            "Local Link frame length is invalid.".to_owned(),
        ));
    }
    let length = u32::try_from(payload.len())
        .map_err(|_| AppError::InvalidInput("Local Link frame is too large.".to_owned()))?;
    stream.write_all(&length.to_be_bytes())?;
    stream.write_all(payload)?;
    stream.flush()?;
    Ok(())
}

fn read_bounded(stream: &mut impl Read, maximum: usize) -> AppResult<Vec<u8>> {
    let mut length = [0_u8; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > maximum {
        return Err(AppError::InvalidInput(
            "Local Link frame length is invalid.".to_owned(),
        ));
    }
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload)?;
    Ok(payload)
}

fn noise_error(_error: snow::Error) -> AppError {
    AppError::InvalidInput("Local Link authentication or encryption failed.".to_owned())
}

#[cfg(test)]
mod tests {
    use std::net::{TcpListener, TcpStream};
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::Duration;

    use super::*;
    use crate::local_link::identity::fingerprint;
    use crate::local_link::state_machine::{
        ReceiptOutcome, ReceiptStateMachine, StatusResolution, TransferPhase,
    };

    #[test]
    fn noise_xx_authenticates_both_static_keys_and_encrypts_messages() {
        let alice = LocalIdentity::generate_for_test().unwrap();
        let bob = LocalIdentity::generate_for_test().unwrap();
        let alice_public = alice.public_key().to_vec();
        let bob_public = bob.public_key().to_vec();
        let (left, right) = UnixStream::pair().unwrap();

        let responder = thread::spawn(move || {
            let mut channel = SecureChannel::responder(right, &bob).unwrap();
            assert_eq!(channel.remote_static(), alice_public);
            let message = channel.receive_message().unwrap();
            channel.send_message(&message).unwrap();
            channel.handshake_hash().to_vec()
        });

        let mut channel = SecureChannel::initiator(left, &alice).unwrap();
        assert_eq!(channel.remote_static(), bob_public);
        let hello = WireMessage::SessionHello {
            version: PROTOCOL_VERSION,
            session_id: uuid::Uuid::new_v4().to_string(),
            device_id: "alice".to_owned(),
            display_name: "Alice Mac".to_owned(),
        };
        channel.send_message(&hello).unwrap();
        assert_eq!(channel.receive_message().unwrap(), hello);
        let initiator_hash = channel.handshake_hash().to_vec();
        assert_eq!(responder.join().unwrap(), initiator_hash);
        assert_eq!(pairing_code(&initiator_hash).len(), 6);
        assert_eq!(transcript_fingerprint(&initiator_hash).len(), 19);
    }

    #[test]
    fn encrypted_payload_is_chunked_and_reassembled_at_the_full_limit() {
        let alice = LocalIdentity::generate_for_test().unwrap();
        let bob = LocalIdentity::generate_for_test().unwrap();
        let (left, right) = UnixStream::pair().unwrap();
        let transfer_id = uuid::Uuid::new_v4().to_string();
        let expected_id = transfer_id.clone();
        let payload = vec![b'x'; MAX_TEXT_PAYLOAD_BYTES];
        let expected = payload.clone();

        let responder = thread::spawn(move || {
            let mut channel = SecureChannel::responder(right, &bob).unwrap();
            channel
                .receive_payload(&expected_id, MAX_TEXT_PAYLOAD_BYTES)
                .unwrap()
        });
        let mut channel = SecureChannel::initiator(left, &alice).unwrap();
        channel.send_payload(&transfer_id, &payload).unwrap();
        assert_eq!(responder.join().unwrap(), expected);
    }

    #[test]
    fn authenticated_cancel_is_accepted_before_the_first_payload_frame() {
        let alice = LocalIdentity::generate_for_test().unwrap();
        let bob = LocalIdentity::generate_for_test().unwrap();
        let (left, right) = UnixStream::pair().unwrap();
        let transfer_id = uuid::Uuid::new_v4().to_string();
        let expected_id = transfer_id.clone();

        let responder = thread::spawn(move || {
            let mut channel = SecureChannel::responder(right, &bob).unwrap();
            channel.receive_payload_or_cancel(&expected_id, 5).unwrap()
        });
        let mut channel = SecureChannel::initiator(left, &alice).unwrap();
        channel
            .send_message(&WireMessage::Cancel { transfer_id })
            .unwrap();
        assert_eq!(responder.join().unwrap(), PayloadOrCancel::Cancelled);
    }

    #[test]
    fn offer_validation_rejects_versions_expiry_and_digest_mismatch() {
        let content = b"hello";
        let mut offer = TransferOffer::new(
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
            content,
        );
        assert_eq!(offer.validate(Utc::now().timestamp_millis()), Ok(()));
        assert_eq!(offer.verify_payload(content), Ok(()));
        assert_eq!(
            offer.verify_payload(b"HELLO"),
            Err(LocalLinkTransferFailure::ProtocolViolation)
        );
        offer.version += 1;
        assert_eq!(
            offer.validate(Utc::now().timestamp_millis()),
            Err(LocalLinkTransferFailure::VersionMismatch)
        );
        offer.version = PROTOCOL_VERSION;
        offer.created_at_ms -= 61_000;
        assert_eq!(
            offer.validate(Utc::now().timestamp_millis()),
            Err(LocalLinkTransferFailure::PayloadUnavailable)
        );
    }

    #[test]
    fn strict_wire_schema_rejects_unknown_fields_and_inconsistent_receipts() {
        let transfer_id = uuid::Uuid::new_v4().to_string();
        let unknown =
            format!(r#"{{"type":"statusQuery","transferId":"{transfer_id}","unexpected":true}}"#);
        assert!(serde_json::from_str::<WireMessage>(&unknown).is_err());
        let nested_unknown = format!(
            r#"{{"type":"offerDecision","transferId":"{transfer_id}","decision":{{"result":"ready","unexpected":true}}}}"#
        );
        assert!(serde_json::from_str::<WireMessage>(&nested_unknown).is_err());

        let invalid_receipt = WireMessage::Receipt {
            transfer_id: transfer_id.clone(),
            receipt: TerminalReceipt {
                outcome: ReceiptOutcome::Copied,
                failure: Some(LocalLinkTransferFailure::DeviceUnavailable),
            },
        };
        assert!(invalid_receipt
            .validate(Utc::now().timestamp_millis())
            .is_err());

        let missing_reason = WireMessage::StatusResponse {
            transfer_id,
            status: StatusResponseState::Terminal(TerminalReceipt {
                outcome: ReceiptOutcome::NotDelivered,
                failure: None,
            }),
        };
        assert!(missing_reason
            .validate(Utc::now().timestamp_millis())
            .is_err());

        assert!(WireMessage::Viewed {
            transfer_id: uuid::Uuid::new_v4().to_string(),
        }
        .validate(Utc::now().timestamp_millis())
        .is_ok());
        assert!(WireMessage::Viewed {
            transfer_id: "not-a-transfer-id".to_owned(),
        }
        .validate(Utc::now().timestamp_millis())
        .is_err());
    }

    #[test]
    fn status_response_v3_has_explicit_content_free_states() {
        assert_eq!(PROTOCOL_VERSION, 3, "status reconciliation requires v3");
        let transfer_id = uuid::Uuid::new_v4().to_string();
        for status in [
            StatusResponseState::Unknown,
            StatusResponseState::Pending,
            StatusResponseState::Terminal(TerminalReceipt::completed(ReceiptOutcome::Copied)),
            StatusResponseState::Terminal(TerminalReceipt::outcome_unknown()),
        ] {
            let message = WireMessage::StatusResponse {
                transfer_id: transfer_id.clone(),
                status,
            };
            message.validate(Utc::now().timestamp_millis()).unwrap();
            let encoded = serde_json::to_string(&message).unwrap();
            let decoded: WireMessage = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, message);
            for forbidden in [
                "body",
                "preview",
                "digest",
                "contentType",
                "deviceId",
                "displayName",
                "endpoint",
            ] {
                assert!(
                    !encoded.contains(forbidden),
                    "status response leaked forbidden field {forbidden}: {encoded}"
                );
            }
        }

        let legacy_optional_receipt =
            format!(r#"{{"type":"statusResponse","transferId":"{transfer_id}","receipt":null}}"#);
        assert!(serde_json::from_str::<WireMessage>(&legacy_optional_receipt).is_err());

        let pending_with_body = format!(
            r#"{{"type":"statusResponse","transferId":"{transfer_id}","status":{{"state":"pending","body":"secret"}}}}"#
        );
        assert!(serde_json::from_str::<WireMessage>(&pending_with_body).is_err());

        for status in [
            StatusResponseState::Unknown,
            StatusResponseState::Pending,
            StatusResponseState::Terminal(TerminalReceipt::completed(ReceiptOutcome::Copied)),
            StatusResponseState::Terminal(TerminalReceipt::outcome_unknown()),
        ] {
            assert!(WireMessage::StatusResponse {
                transfer_id: "not-a-transfer-id".to_owned(),
                status,
            }
            .validate(Utc::now().timestamp_millis())
            .is_err());
        }
    }

    #[test]
    fn status_control_messages_round_trip_over_an_authenticated_noise_socket() {
        let alice = LocalIdentity::generate_for_test().unwrap();
        let bob = LocalIdentity::generate_for_test().unwrap();
        let alice_public = alice.public_key().to_vec();
        let bob_public = bob.public_key().to_vec();
        let transfer_ids = [
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
        ];
        let responder_ids = transfer_ids.clone();
        let (left, right) = UnixStream::pair().unwrap();

        let responder = thread::spawn(move || {
            let mut channel = SecureChannel::responder(right, &bob).unwrap();
            assert_eq!(channel.remote_static(), alice_public);
            let responses = [
                StatusResponseState::Unknown,
                StatusResponseState::Pending,
                StatusResponseState::Terminal(TerminalReceipt::completed(ReceiptOutcome::Saved)),
            ];
            for (expected_id, status) in responder_ids.into_iter().zip(responses) {
                let WireMessage::StatusQuery { transfer_id } = channel.receive_message().unwrap()
                else {
                    panic!("expected status query");
                };
                assert_eq!(transfer_id, expected_id);
                channel
                    .send_message(&WireMessage::StatusResponse {
                        transfer_id,
                        status,
                    })
                    .unwrap();
            }
        });

        let mut channel = SecureChannel::initiator(left, &alice).unwrap();
        assert_eq!(channel.remote_static(), bob_public);
        let expected_states = [
            StatusResponseState::Unknown,
            StatusResponseState::Pending,
            StatusResponseState::Terminal(TerminalReceipt::completed(ReceiptOutcome::Saved)),
        ];
        for (transfer_id, expected) in transfer_ids.into_iter().zip(expected_states) {
            channel
                .send_message(&WireMessage::StatusQuery {
                    transfer_id: transfer_id.clone(),
                })
                .unwrap();
            let WireMessage::StatusResponse {
                transfer_id: response_id,
                status,
            } = channel.receive_message().unwrap()
            else {
                panic!("expected status response");
            };
            assert_eq!(response_id, transfer_id);
            assert_eq!(status, expected);
        }
        responder.join().unwrap();
    }

    #[test]
    fn encrypted_channel_rejects_legacy_or_malformed_status_responses() {
        let alice = LocalIdentity::generate_for_test().unwrap();
        let bob = LocalIdentity::generate_for_test().unwrap();
        let transfer_id = uuid::Uuid::new_v4().to_string();
        let (left, right) = UnixStream::pair().unwrap();

        let responder = thread::spawn(move || {
            let mut channel = SecureChannel::responder(right, &bob).unwrap();
            channel.receive_message()
        });
        let mut channel = SecureChannel::initiator(left, &alice).unwrap();
        let legacy =
            format!(r#"{{"type":"statusResponse","transferId":"{transfer_id}","receipt":null}}"#);
        channel.send_frame(FRAME_JSON, legacy.as_bytes()).unwrap();
        assert!(responder.join().unwrap().is_err());

        let invalid_terminal = WireMessage::StatusResponse {
            transfer_id: uuid::Uuid::new_v4().to_string(),
            status: StatusResponseState::Terminal(TerminalReceipt {
                outcome: ReceiptOutcome::Copied,
                failure: Some(LocalLinkTransferFailure::DeviceUnavailable),
            }),
        };
        assert!(invalid_terminal
            .validate(Utc::now().timestamp_millis())
            .is_err());

        let old_hello = WireMessage::SessionHello {
            version: PROTOCOL_VERSION - 1,
            session_id: uuid::Uuid::new_v4().to_string(),
            device_id: "old-peer".to_owned(),
            display_name: "Old Peer".to_owned(),
        };
        assert!(old_hello.validate(Utc::now().timestamp_millis()).is_err());
    }

    #[test]
    fn pairing_messages_are_bounded_and_transcript_scoped() {
        let now_ms = Utc::now().timestamp_millis();
        let mut proposal = PairingProposal {
            version: PROTOCOL_VERSION,
            pairing_id: uuid::Uuid::new_v4().to_string(),
            session_id: uuid::Uuid::new_v4().to_string(),
            device_id: "peer-static-id".to_owned(),
            display_name: "Peer Mac".to_owned(),
            public_key_fingerprint: "ABCD-EF01-2345-6789-ABCD-EF01".to_owned(),
            expires_at_ms: now_ms + 59_000,
        };
        assert!(WireMessage::PairingProposal(proposal.clone())
            .validate(now_ms)
            .is_ok());
        proposal.display_name = "x".repeat(MAX_DISPLAY_NAME_CHARS + 1);
        assert!(WireMessage::PairingProposal(proposal)
            .validate(now_ms)
            .is_err());
    }

    fn preview_payload(index: usize) -> Vec<u8> {
        match index {
            0 => vec![b'x'],
            1 => "中文、emoji🙂、URL https://clipriva.invalid/\nfn main() {}"
                .as_bytes()
                .to_vec(),
            199 => vec![b'z'; MAX_TEXT_PAYLOAD_BYTES],
            _ => format!("预门槛 payload {index:03} 🙂\nline two\nhttps://example.invalid/{index}")
                .into_bytes(),
        }
    }

    fn preview_receipt(index: usize) -> TerminalReceipt {
        match index % 7 {
            0 => TerminalReceipt::completed(ReceiptOutcome::Copied),
            1 => TerminalReceipt::completed(ReceiptOutcome::Saved),
            2 => TerminalReceipt::completed(ReceiptOutcome::Rejected),
            3 => TerminalReceipt::completed(ReceiptOutcome::Cancelled),
            4 => TerminalReceipt::completed(ReceiptOutcome::Expired),
            5 => TerminalReceipt::outcome_unknown(),
            _ => TerminalReceipt::not_delivered(LocalLinkTransferFailure::DeviceUnavailable),
        }
    }

    /// This validates the protocol codec over a real TCP socket with two
    /// independent identities; production transport integration is tested in
    /// the Local Link service module.
    #[test]
    fn preview_pre_gate_real_tcp_sas_pin_200_payloads_and_receipt_convergence() {
        let alice = LocalIdentity::generate_for_test().unwrap();
        let bob = LocalIdentity::generate_for_test().unwrap();
        let alice_public = alice.public_key().to_vec();
        let bob_public = bob.public_key().to_vec();
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let pairing_id = uuid::Uuid::new_v4().to_string();
        let session_id = uuid::Uuid::new_v4().to_string();
        let responder_pairing_id = pairing_id.clone();
        let responder_session_id = session_id.clone();

        let responder = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut channel = SecureChannel::responder(stream, &bob).unwrap();
            assert_eq!(channel.remote_static(), alice_public);
            let responder_hash = channel.handshake_hash().to_vec();
            let sas = pairing_code(&responder_hash);
            let transcript = transcript_fingerprint(&responder_hash);

            let WireMessage::SessionHello { device_id, .. } = channel.receive_message().unwrap()
            else {
                panic!("expected initiator hello");
            };
            assert_eq!(device_id, "alice-static-id");
            let WireMessage::PairingProposal(proposal) = channel.receive_message().unwrap() else {
                panic!("expected initiator pairing proposal");
            };
            assert_eq!(proposal.pairing_id, responder_pairing_id);
            assert_eq!(proposal.session_id, responder_session_id);
            channel
                .send_message(&WireMessage::SessionHello {
                    version: PROTOCOL_VERSION,
                    session_id: responder_session_id.clone(),
                    device_id: "bob-static-id".to_owned(),
                    display_name: "Bob Mac".to_owned(),
                })
                .unwrap();
            channel
                .send_message(&WireMessage::PairingProposal(PairingProposal {
                    version: PROTOCOL_VERSION,
                    pairing_id: responder_pairing_id.clone(),
                    session_id: responder_session_id.clone(),
                    device_id: "bob-static-id".to_owned(),
                    display_name: "Bob Mac".to_owned(),
                    public_key_fingerprint: fingerprint(bob.public_key()),
                    expires_at_ms: Utc::now().timestamp_millis() + 59_000,
                }))
                .unwrap();
            let WireMessage::PairingConfirm(confirm) = channel.receive_message().unwrap() else {
                panic!("expected initiator confirmation");
            };
            assert!(confirm.confirmed);
            assert_eq!(confirm.transcript_fingerprint, transcript);
            channel
                .send_message(&WireMessage::PairingConfirm(PairingConfirm {
                    version: PROTOCOL_VERSION,
                    pairing_id: responder_pairing_id.clone(),
                    session_id: responder_session_id.clone(),
                    transcript_fingerprint: transcript.clone(),
                    confirmed: true,
                }))
                .unwrap();
            let WireMessage::PairingCommit(commit) = channel.receive_message().unwrap() else {
                panic!("expected initiator commit");
            };
            assert_eq!(commit.public_key_fingerprint, fingerprint(&alice_public));
            channel
                .send_message(&WireMessage::PairingCommit(PairingCommit {
                    version: PROTOCOL_VERSION,
                    pairing_id: responder_pairing_id,
                    session_id: responder_session_id,
                    device_id: "bob-static-id".to_owned(),
                    public_key_fingerprint: fingerprint(bob.public_key()),
                }))
                .unwrap();

            let mut receipts = ReceiptStateMachine::default();
            for index in 0..200 {
                let WireMessage::Offer(offer) = channel.receive_message().unwrap() else {
                    panic!("expected transfer offer");
                };
                let expected = preview_payload(index);
                assert_eq!(offer.byte_size as usize, expected.len());
                channel
                    .send_message(&WireMessage::OfferDecision {
                        transfer_id: offer.transfer_id.clone(),
                        decision: OfferDecision::Ready,
                    })
                    .unwrap();
                let payload = channel
                    .receive_payload(&offer.transfer_id, offer.byte_size as usize)
                    .unwrap();
                offer.verify_payload(&payload).unwrap();
                assert_eq!(payload, expected);
                let WireMessage::PayloadComplete { transfer_id } =
                    channel.receive_message().unwrap()
                else {
                    panic!("expected payload completion");
                };
                assert_eq!(transfer_id, offer.transfer_id);
                let receipt = preview_receipt(index);
                receipts.begin(offer.transfer_id.clone());
                receipts.advance(&offer.transfer_id, TransferPhase::Encrypted);
                receipts.advance(&offer.transfer_id, TransferPhase::AwaitingReceiver);
                receipts.apply_receipt(&offer.transfer_id, receipt);

                if index % 5 == 0 {
                    let WireMessage::StatusQuery { transfer_id } =
                        channel.receive_message().unwrap()
                    else {
                        panic!("expected status query after omitted receipt");
                    };
                    let StatusResolution::Terminal(stored) = receipts.status(&transfer_id) else {
                        panic!("terminal receipt must be retained for convergence");
                    };
                    channel
                        .send_message(&WireMessage::StatusResponse {
                            transfer_id,
                            status: StatusResponseState::Terminal(stored),
                        })
                        .unwrap();
                } else {
                    channel
                        .send_message(&WireMessage::Receipt {
                            transfer_id: offer.transfer_id.clone(),
                            receipt,
                        })
                        .unwrap();
                    let WireMessage::ReceiptAck {
                        transfer_id,
                        receipt: acknowledged,
                    } = channel.receive_message().unwrap()
                    else {
                        panic!("expected receipt acknowledgement");
                    };
                    assert_eq!(transfer_id, offer.transfer_id);
                    assert_eq!(acknowledged, receipt);
                    if index % 7 == 0 {
                        channel
                            .send_message(&WireMessage::Receipt {
                                transfer_id: offer.transfer_id.clone(),
                                receipt,
                            })
                            .unwrap();
                        let WireMessage::ReceiptAck {
                            receipt: duplicate, ..
                        } = channel.receive_message().unwrap()
                        else {
                            panic!("expected duplicate receipt acknowledgement");
                        };
                        assert_eq!(duplicate, receipt);
                    }
                }
            }
            (sas, responder_hash)
        });

        let stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut channel = SecureChannel::initiator(stream, &alice).unwrap();
        assert_eq!(channel.remote_static(), bob_public);
        let initiator_hash = channel.handshake_hash().to_vec();
        let initiator_sas = pairing_code(&initiator_hash);
        let transcript = transcript_fingerprint(&initiator_hash);
        channel
            .send_message(&WireMessage::SessionHello {
                version: PROTOCOL_VERSION,
                session_id: session_id.clone(),
                device_id: "alice-static-id".to_owned(),
                display_name: "Alice Mac".to_owned(),
            })
            .unwrap();
        channel
            .send_message(&WireMessage::PairingProposal(PairingProposal {
                version: PROTOCOL_VERSION,
                pairing_id: pairing_id.clone(),
                session_id: session_id.clone(),
                device_id: "alice-static-id".to_owned(),
                display_name: "Alice Mac".to_owned(),
                public_key_fingerprint: fingerprint(alice.public_key()),
                expires_at_ms: Utc::now().timestamp_millis() + 59_000,
            }))
            .unwrap();
        let WireMessage::SessionHello { device_id, .. } = channel.receive_message().unwrap() else {
            panic!("expected responder hello");
        };
        assert_eq!(device_id, "bob-static-id");
        let WireMessage::PairingProposal(proposal) = channel.receive_message().unwrap() else {
            panic!("expected responder pairing proposal");
        };
        assert_eq!(proposal.public_key_fingerprint, fingerprint(&bob_public));
        channel
            .send_message(&WireMessage::PairingConfirm(PairingConfirm {
                version: PROTOCOL_VERSION,
                pairing_id: pairing_id.clone(),
                session_id: session_id.clone(),
                transcript_fingerprint: transcript.clone(),
                confirmed: true,
            }))
            .unwrap();
        let WireMessage::PairingConfirm(confirm) = channel.receive_message().unwrap() else {
            panic!("expected responder confirmation");
        };
        assert!(confirm.confirmed);
        assert_eq!(confirm.transcript_fingerprint, transcript);
        channel
            .send_message(&WireMessage::PairingCommit(PairingCommit {
                version: PROTOCOL_VERSION,
                pairing_id: pairing_id.clone(),
                session_id: session_id.clone(),
                device_id: "alice-static-id".to_owned(),
                public_key_fingerprint: fingerprint(alice.public_key()),
            }))
            .unwrap();
        let WireMessage::PairingCommit(commit) = channel.receive_message().unwrap() else {
            panic!("expected responder commit");
        };
        assert_eq!(commit.pairing_id, pairing_id);
        assert_eq!(commit.session_id, session_id);
        assert_eq!(commit.public_key_fingerprint, fingerprint(&bob_public));

        let mut receipts = ReceiptStateMachine::default();
        for index in 0..200 {
            let payload = preview_payload(index);
            let transfer_id = uuid::Uuid::new_v4().to_string();
            let offer = TransferOffer::new(
                uuid::Uuid::new_v4().to_string(),
                transfer_id.clone(),
                &payload,
            );
            receipts.begin(transfer_id.clone());
            channel
                .send_message(&WireMessage::Offer(offer.clone()))
                .unwrap();
            let WireMessage::OfferDecision {
                transfer_id: accepted_id,
                decision: OfferDecision::Ready,
            } = channel.receive_message().unwrap()
            else {
                panic!("expected ready decision");
            };
            assert_eq!(accepted_id, transfer_id);
            receipts.advance(&transfer_id, TransferPhase::Encrypted);
            channel.send_payload(&transfer_id, &payload).unwrap();
            channel
                .send_message(&WireMessage::PayloadComplete {
                    transfer_id: transfer_id.clone(),
                })
                .unwrap();
            receipts.advance(&transfer_id, TransferPhase::AwaitingReceiver);
            let expected = preview_receipt(index);

            if index % 5 == 0 {
                channel
                    .send_message(&WireMessage::StatusQuery {
                        transfer_id: transfer_id.clone(),
                    })
                    .unwrap();
                let WireMessage::StatusResponse {
                    transfer_id: response_id,
                    status: StatusResponseState::Terminal(receipt),
                } = channel.receive_message().unwrap()
                else {
                    panic!("expected converged status response");
                };
                assert_eq!(response_id, transfer_id);
                assert_eq!(receipt, expected);
                assert_eq!(
                    receipts.resolve_status(&transfer_id, Some(receipt)),
                    StatusResolution::Terminal(expected)
                );
            } else {
                let WireMessage::Receipt {
                    transfer_id: receipt_id,
                    receipt,
                } = channel.receive_message().unwrap()
                else {
                    panic!("expected receipt");
                };
                assert_eq!(receipt_id, transfer_id);
                assert_eq!(receipt, expected);
                receipts.apply_receipt(&transfer_id, receipt);
                channel
                    .send_message(&WireMessage::ReceiptAck {
                        transfer_id: transfer_id.clone(),
                        receipt,
                    })
                    .unwrap();
                if index % 7 == 0 {
                    let WireMessage::Receipt {
                        receipt: duplicate, ..
                    } = channel.receive_message().unwrap()
                    else {
                        panic!("expected duplicate receipt");
                    };
                    assert_eq!(duplicate, expected);
                    receipts.apply_receipt(&transfer_id, duplicate);
                    channel
                        .send_message(&WireMessage::ReceiptAck {
                            transfer_id: transfer_id.clone(),
                            receipt: duplicate,
                        })
                        .unwrap();
                }
                assert_eq!(
                    receipts.status(&transfer_id),
                    StatusResolution::Terminal(expected)
                );
            }
        }

        let (responder_sas, responder_hash) = responder.join().unwrap();
        assert_eq!(responder_sas, initiator_sas);
        assert_eq!(responder_hash, initiator_hash);
    }
}
