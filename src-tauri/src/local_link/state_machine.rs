//! Single-winner Local Link transfer state and receipt convergence.
//!
//! This module is deliberately transport-agnostic. Every transport adapter
//! must use this state machine instead of assembling terminal state from
//! independent booleans.

#![allow(
    dead_code,
    reason = "transition results are consumed by the Preview transport pre-gate while the production listener remains fail-closed"
)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::models::LocalLinkTransferFailure;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ReceiptOutcome {
    Copied,
    Saved,
    Rejected,
    Cancelled,
    Expired,
    OutcomeUnknown,
    NotDelivered,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TerminalReceipt {
    pub outcome: ReceiptOutcome,
    pub failure: Option<LocalLinkTransferFailure>,
}

impl TerminalReceipt {
    pub(crate) fn completed(outcome: ReceiptOutcome) -> Self {
        debug_assert!(!matches!(
            outcome,
            ReceiptOutcome::NotDelivered | ReceiptOutcome::OutcomeUnknown
        ));
        Self {
            outcome,
            failure: None,
        }
    }

    pub(crate) fn outcome_unknown() -> Self {
        Self {
            outcome: ReceiptOutcome::OutcomeUnknown,
            failure: None,
        }
    }

    pub(crate) fn not_delivered(failure: LocalLinkTransferFailure) -> Self {
        Self {
            outcome: ReceiptOutcome::NotDelivered,
            failure: Some(failure),
        }
    }

    pub(crate) fn is_valid(self) -> bool {
        matches!(
            (self.outcome, self.failure),
            (ReceiptOutcome::NotDelivered, Some(_))
                | (
                    ReceiptOutcome::Copied
                        | ReceiptOutcome::Saved
                        | ReceiptOutcome::Rejected
                        | ReceiptOutcome::Cancelled
                        | ReceiptOutcome::Expired
                        | ReceiptOutcome::OutcomeUnknown,
                    None
                )
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransferPhase {
    Connecting,
    Encrypted,
    AwaitingReceiver,
    Viewed,
    Terminal(TerminalReceipt),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransitionResult {
    Applied(TransferPhase),
    Idempotent(TransferPhase),
    Rejected { current: TransferPhase },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReceiptApplyResult {
    Applied(TerminalReceipt),
    Idempotent(TerminalReceipt),
    Conflict { current: TerminalReceipt },
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusResolution {
    Unknown,
    Pending(TransferPhase),
    Terminal(TerminalReceipt),
}

#[derive(Default)]
pub(crate) struct ReceiptStateMachine {
    transfers: HashMap<String, TransferPhase>,
}

impl ReceiptStateMachine {
    pub(crate) fn begin(&mut self, transfer_id: impl Into<String>) -> TransitionResult {
        let transfer_id = transfer_id.into();
        match self.transfers.get(&transfer_id).copied() {
            None => {
                self.transfers
                    .insert(transfer_id, TransferPhase::Connecting);
                TransitionResult::Applied(TransferPhase::Connecting)
            }
            Some(current) => TransitionResult::Idempotent(current),
        }
    }

    pub(crate) fn advance(&mut self, transfer_id: &str, next: TransferPhase) -> TransitionResult {
        let Some(current) = self.transfers.get(transfer_id).copied() else {
            return TransitionResult::Rejected {
                current: TransferPhase::Connecting,
            };
        };
        if current == next {
            return TransitionResult::Idempotent(current);
        }
        let legal = matches!(
            (current, next),
            (TransferPhase::Connecting, TransferPhase::Encrypted)
                | (TransferPhase::Encrypted, TransferPhase::AwaitingReceiver)
                | (TransferPhase::AwaitingReceiver, TransferPhase::Viewed)
        );
        if !legal {
            return TransitionResult::Rejected { current };
        }
        self.transfers.insert(transfer_id.to_owned(), next);
        TransitionResult::Applied(next)
    }

    /// Apply an authoritative terminal receipt. Receipt delivery may be late,
    /// duplicated or arrive after intermediate phase updates were lost, so any
    /// active phase may converge directly to a terminal receipt. Once terminal,
    /// only the identical receipt is idempotent; a different result is rejected.
    pub(crate) fn apply_receipt(
        &mut self,
        transfer_id: &str,
        receipt: TerminalReceipt,
    ) -> ReceiptApplyResult {
        if !receipt.is_valid() {
            return ReceiptApplyResult::Invalid;
        }
        match self.transfers.get(transfer_id).copied() {
            Some(TransferPhase::Terminal(current)) if current == receipt => {
                ReceiptApplyResult::Idempotent(current)
            }
            Some(TransferPhase::Terminal(current)) => ReceiptApplyResult::Conflict { current },
            Some(_) | None => {
                self.transfers
                    .insert(transfer_id.to_owned(), TransferPhase::Terminal(receipt));
                ReceiptApplyResult::Applied(receipt)
            }
        }
    }

    /// Reconcile a terminal receipt sent by the receiver after a local sender
    /// cancellation or terminal uncertainty was recorded locally. The
    /// receiver's authenticated decision is authoritative when it had already
    /// won before `Cancel`, revocation, shutdown, or the bounded reconciliation
    /// deadline. These are the only exceptions to the otherwise immutable
    /// terminal-state rule.
    pub(crate) fn apply_remote_receipt(
        &mut self,
        transfer_id: &str,
        receipt: TerminalReceipt,
    ) -> ReceiptApplyResult {
        if !receipt.is_valid() {
            return ReceiptApplyResult::Invalid;
        }
        match self.transfers.get(transfer_id).copied() {
            Some(TransferPhase::Terminal(current))
                if matches!(
                    current.outcome,
                    ReceiptOutcome::Cancelled | ReceiptOutcome::OutcomeUnknown
                ) && current != receipt =>
            {
                self.transfers
                    .insert(transfer_id.to_owned(), TransferPhase::Terminal(receipt));
                ReceiptApplyResult::Applied(receipt)
            }
            _ => self.apply_receipt(transfer_id, receipt),
        }
    }

    /// Resolve a StatusResponse. `None` never rolls back known local state; it
    /// simply reports the best state already known by this side.
    pub(crate) fn resolve_status(
        &mut self,
        transfer_id: &str,
        remote_receipt: Option<TerminalReceipt>,
    ) -> StatusResolution {
        if let Some(receipt) = remote_receipt {
            let _ = self.apply_receipt(transfer_id, receipt);
        }
        self.status(transfer_id)
    }

    pub(crate) fn status(&self, transfer_id: &str) -> StatusResolution {
        match self.transfers.get(transfer_id).copied() {
            None => StatusResolution::Unknown,
            Some(TransferPhase::Terminal(receipt)) => StatusResolution::Terminal(receipt),
            Some(phase) => StatusResolution::Pending(phase),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.transfers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_only_move_forward_and_terminal_is_single_winner() {
        let mut machine = ReceiptStateMachine::default();
        let copied = TerminalReceipt::completed(ReceiptOutcome::Copied);
        let saved = TerminalReceipt::completed(ReceiptOutcome::Saved);
        assert_eq!(
            machine.begin("transfer"),
            TransitionResult::Applied(TransferPhase::Connecting)
        );
        assert_eq!(
            machine.advance("transfer", TransferPhase::AwaitingReceiver),
            TransitionResult::Rejected {
                current: TransferPhase::Connecting
            }
        );
        assert_eq!(
            machine.advance("transfer", TransferPhase::Encrypted),
            TransitionResult::Applied(TransferPhase::Encrypted)
        );
        assert_eq!(
            machine.advance("transfer", TransferPhase::AwaitingReceiver),
            TransitionResult::Applied(TransferPhase::AwaitingReceiver)
        );
        assert_eq!(
            machine.apply_receipt("transfer", copied),
            ReceiptApplyResult::Applied(copied)
        );
        assert_eq!(
            machine.apply_receipt("transfer", copied),
            ReceiptApplyResult::Idempotent(copied)
        );
        assert_eq!(
            machine.apply_receipt("transfer", saved),
            ReceiptApplyResult::Conflict { current: copied }
        );
    }

    #[test]
    fn lost_duplicate_and_out_of_order_receipts_converge() {
        let mut sender = ReceiptStateMachine::default();
        let saved = TerminalReceipt::completed(ReceiptOutcome::Saved);
        let expired = TerminalReceipt::completed(ReceiptOutcome::Expired);
        sender.begin("lost");
        sender.advance("lost", TransferPhase::Encrypted);

        // The original Receipt is lost. A later StatusResponse is authoritative
        // and can skip a missing intermediate phase update.
        assert_eq!(
            sender.resolve_status("lost", Some(saved)),
            StatusResolution::Terminal(saved)
        );
        assert_eq!(
            sender.resolve_status("lost", Some(saved)),
            StatusResolution::Terminal(saved)
        );

        // A reordered stale response cannot replace the first terminal winner.
        assert_eq!(
            sender.resolve_status("lost", Some(expired)),
            StatusResolution::Terminal(saved)
        );

        // An empty/unknown response cannot roll terminal state backwards.
        assert_eq!(
            sender.resolve_status("lost", None),
            StatusResolution::Terminal(saved)
        );
    }

    #[test]
    fn only_not_delivered_can_carry_a_failure_reason() {
        let invalid = TerminalReceipt {
            outcome: ReceiptOutcome::Copied,
            failure: Some(LocalLinkTransferFailure::DeviceUnavailable),
        };
        assert!(!invalid.is_valid());
        let missing_reason = TerminalReceipt {
            outcome: ReceiptOutcome::NotDelivered,
            failure: None,
        };
        assert!(!missing_reason.is_valid());
        assert!(
            TerminalReceipt::not_delivered(LocalLinkTransferFailure::DeviceUnavailable).is_valid()
        );
        assert!(TerminalReceipt::outcome_unknown().is_valid());
        assert!(!TerminalReceipt {
            outcome: ReceiptOutcome::OutcomeUnknown,
            failure: Some(LocalLinkTransferFailure::OutcomeUnknown),
        }
        .is_valid());
    }

    #[test]
    fn viewed_is_a_single_metadata_transition_before_a_terminal_decision() {
        let mut machine = ReceiptStateMachine::default();
        let copied = TerminalReceipt::completed(ReceiptOutcome::Copied);
        machine.begin("viewed");
        machine.advance("viewed", TransferPhase::Encrypted);
        machine.advance("viewed", TransferPhase::AwaitingReceiver);

        assert_eq!(
            machine.advance("viewed", TransferPhase::Viewed),
            TransitionResult::Applied(TransferPhase::Viewed)
        );
        assert_eq!(
            machine.advance("viewed", TransferPhase::Viewed),
            TransitionResult::Idempotent(TransferPhase::Viewed)
        );
        assert_eq!(
            machine.apply_receipt("viewed", copied),
            ReceiptApplyResult::Applied(copied)
        );
        assert_eq!(
            machine.advance("viewed", TransferPhase::AwaitingReceiver),
            TransitionResult::Rejected {
                current: TransferPhase::Terminal(copied)
            }
        );
    }

    #[test]
    fn receiver_receipt_can_reconcile_a_provisional_sender_cancellation() {
        let mut machine = ReceiptStateMachine::default();
        let cancelled = TerminalReceipt::completed(ReceiptOutcome::Cancelled);
        let copied = TerminalReceipt::completed(ReceiptOutcome::Copied);
        machine.begin("cancel-race");
        machine.advance("cancel-race", TransferPhase::Encrypted);
        machine.advance("cancel-race", TransferPhase::AwaitingReceiver);
        assert_eq!(
            machine.apply_receipt("cancel-race", cancelled),
            ReceiptApplyResult::Applied(cancelled)
        );

        // The receiver had already copied before it observed Cancel. Its
        // duplicate terminal receipt remains idempotent after reconciliation.
        assert_eq!(
            machine.apply_remote_receipt("cancel-race", copied),
            ReceiptApplyResult::Applied(copied)
        );
        assert_eq!(
            machine.apply_remote_receipt("cancel-race", copied),
            ReceiptApplyResult::Idempotent(copied)
        );
        assert_eq!(
            machine.apply_remote_receipt("cancel-race", cancelled),
            ReceiptApplyResult::Conflict { current: copied }
        );
    }

    #[test]
    fn authenticated_receiver_receipt_replaces_local_outcome_unknown() {
        let mut machine = ReceiptStateMachine::default();
        let copied = TerminalReceipt::completed(ReceiptOutcome::Copied);
        machine.begin("unknown-race");
        assert_eq!(
            machine.apply_receipt("unknown-race", TerminalReceipt::outcome_unknown()),
            ReceiptApplyResult::Applied(TerminalReceipt::outcome_unknown())
        );
        assert_eq!(
            machine.apply_remote_receipt("unknown-race", copied),
            ReceiptApplyResult::Applied(copied)
        );
    }
}
