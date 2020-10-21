// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::{format_err, Error, Result};
use libra_crypto::HashValue;
use libra_types::{
    account_config::{
        AccountResource, AccountRole, BalanceResource, BaseUrlRotationEvent, BurnEvent,
        CancelBurnEvent, ComplianceKeyRotationEvent, CreateAccountEvent, CurrencyInfoResource,
        FreezingBit, MintEvent, NewBlockEvent, NewEpochEvent, PreburnEvent, ReceivedMintEvent,
        ReceivedPaymentEvent, SentPaymentEvent, ToLBRExchangeRateUpdateEvent, UpgradeEvent,
    },
    account_state_blob::AccountStateWithProof,
    contract_event::ContractEvent,
    epoch_change::EpochChangeProof,
    ledger_info::LedgerInfoWithSignatures,
    proof::{AccountStateProof, AccumulatorConsistencyProof},
    transaction::{Transaction, TransactionArgument, TransactionPayload},
    vm_status::KeptVMStatus,
};
use move_core_types::{
    account_address::AccountAddress,
    identifier::Identifier,
    language_storage::{StructTag, TypeTag},
    move_resource::MoveResource,
    vm_status::AbortLocation,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    convert::{TryFrom, TryInto},
};
use transaction_builder::get_transaction_name;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct AmountView {
    pub amount: u64,
    pub currency: String,
}

impl AmountView {
    fn new(amount: u64, currency: &str) -> Self {
        Self {
            amount,
            currency: currency.to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum AccountRoleView {
    #[serde(rename = "unknown")]
    Unknown,
    #[serde(rename = "child_vasp")]
    ChildVASP { parent_vasp_address: BytesView },
    #[serde(rename = "parent_vasp")]
    ParentVASP {
        human_name: String,
        base_url: String,
        expiration_time: u64,
        compliance_key: BytesView,
        num_children: u64,
        compliance_key_rotation_events_key: BytesView,
        base_url_rotation_events_key: BytesView,
    },
    #[serde(rename = "designated_dealer")]
    DesignatedDealer {
        human_name: String,
        base_url: String,
        expiration_time: u64,
        compliance_key: BytesView,
        preburn_balances: Vec<AmountView>,
        received_mint_events_key: BytesView,
        compliance_key_rotation_events_key: BytesView,
        base_url_rotation_events_key: BytesView,
    },
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct AccountView {
    pub address: BytesView,
    pub balances: Vec<AmountView>,
    pub sequence_number: u64,
    pub authentication_key: BytesView,
    pub sent_events_key: BytesView,
    pub received_events_key: BytesView,
    pub delegated_key_rotation_capability: bool,
    pub delegated_withdrawal_capability: bool,
    pub is_frozen: bool,
    pub role: AccountRoleView,
}

impl AccountView {
    pub fn new(
        address: &AccountAddress,
        account: &AccountResource,
        balances: BTreeMap<Identifier, BalanceResource>,
        account_role: AccountRole,
        freezing_bit: FreezingBit,
    ) -> Self {
        Self {
            address: BytesView::from(address.to_vec()),
            balances: balances
                .iter()
                .map(|(currency_code, balance)| {
                    AmountView::new(balance.coin(), &currency_code.as_str())
                })
                .collect(),
            sequence_number: account.sequence_number(),
            authentication_key: BytesView::from(account.authentication_key()),
            sent_events_key: BytesView::from(account.sent_events().key().as_bytes()),
            received_events_key: BytesView::from(account.received_events().key().as_bytes()),
            delegated_key_rotation_capability: account.has_delegated_key_rotation_capability(),
            delegated_withdrawal_capability: account.has_delegated_withdrawal_capability(),
            is_frozen: freezing_bit.is_frozen(),
            role: AccountRoleView::from(account_role),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EventView {
    pub key: BytesView,
    pub sequence_number: u64,
    pub transaction_version: u64,
    pub data: EventDataView,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum EventDataView {
    #[serde(rename = "burn")]
    Burn {
        amount: AmountView,
        preburn_address: BytesView,
    },
    #[serde(rename = "cancelburn")]
    CancelBurn {
        amount: AmountView,
        preburn_address: BytesView,
    },
    #[serde(rename = "mint")]
    Mint { amount: AmountView },
    #[serde(rename = "to_lbr_exchange_rate_update")]
    ToLBRExchangeRateUpdate {
        currency_code: String,
        new_to_lbr_exchange_rate: f32,
    },
    #[serde(rename = "preburn")]
    Preburn {
        amount: AmountView,
        preburn_address: BytesView,
    },
    #[serde(rename = "receivedpayment")]
    ReceivedPayment {
        amount: AmountView,
        sender: BytesView,
        receiver: BytesView,
        metadata: BytesView,
    },
    #[serde(rename = "sentpayment")]
    SentPayment {
        amount: AmountView,
        receiver: BytesView,
        sender: BytesView,
        metadata: BytesView,
    },
    #[serde(rename = "upgrade")]
    Upgrade { write_set: BytesView },
    #[serde(rename = "newepoch")]
    NewEpoch { epoch: u64 },
    #[serde(rename = "newblock")]
    NewBlock {
        round: u64,
        proposer: BytesView,
        proposed_time: u64,
    },
    #[serde(rename = "receivedmint")]
    ReceivedMint {
        amount: AmountView,
        destination_address: BytesView,
    },
    #[serde(rename = "compliancekeyrotation")]
    ComplianceKeyRotation {
        new_compliance_public_key: String,
        time_rotated_seconds: u64,
    },
    #[serde(rename = "baseurlrotation")]
    BaseUrlRotation {
        new_base_url: String,
        time_rotated_seconds: u64,
    },
    #[serde(rename = "createaccount")]
    CreateAccount {
        created_address: BytesView,
        role_id: u64,
    },
    #[serde(rename = "unknown")]
    Unknown {},
}

impl TryFrom<ContractEvent> for EventDataView {
    type Error = Error;

    fn try_from(event: ContractEvent) -> Result<Self> {
        let data = if event.type_tag() == &TypeTag::Struct(ReceivedPaymentEvent::struct_tag()) {
            let received_event = ReceivedPaymentEvent::try_from(&event)?;
            let amount_view = AmountView::new(
                received_event.amount(),
                received_event.currency_code().as_str(),
            );
            EventDataView::ReceivedPayment {
                amount: amount_view,
                sender: BytesView::from(received_event.sender().as_ref()),
                receiver: BytesView::from(&event.key().get_creator_address().to_vec()),
                metadata: BytesView::from(received_event.metadata()),
            }
        } else if event.type_tag() == &TypeTag::Struct(SentPaymentEvent::struct_tag()) {
            let sent_event = SentPaymentEvent::try_from(&event)?;
            let amount_view =
                AmountView::new(sent_event.amount(), sent_event.currency_code().as_str());
            EventDataView::SentPayment {
                amount: amount_view,
                receiver: BytesView::from(sent_event.receiver().as_ref()),
                sender: BytesView::from(&event.key().get_creator_address().to_vec()),
                metadata: BytesView::from(sent_event.metadata()),
            }
        } else if event.type_tag() == &TypeTag::Struct(PreburnEvent::struct_tag()) {
            let preburn_event = PreburnEvent::try_from(&event)?;
            let amount_view = AmountView::new(
                preburn_event.amount(),
                preburn_event.currency_code().as_str(),
            );
            let preburn_address = BytesView::from(preburn_event.preburn_address().as_ref());
            EventDataView::Preburn {
                amount: amount_view,
                preburn_address,
            }
        } else if event.type_tag() == &TypeTag::Struct(BurnEvent::struct_tag()) {
            let burn_event = BurnEvent::try_from(&event)?;
            let amount_view =
                AmountView::new(burn_event.amount(), burn_event.currency_code().as_str());
            let preburn_address = BytesView::from(burn_event.preburn_address().as_ref());
            EventDataView::Burn {
                amount: amount_view,
                preburn_address,
            }
        } else if event.type_tag() == &TypeTag::Struct(CancelBurnEvent::struct_tag()) {
            let cancel_burn_event = CancelBurnEvent::try_from(&event)?;
            let amount_view = AmountView::new(
                cancel_burn_event.amount(),
                cancel_burn_event.currency_code().as_str(),
            );
            let preburn_address = BytesView::from(cancel_burn_event.preburn_address().as_ref());
            EventDataView::CancelBurn {
                amount: amount_view,
                preburn_address,
            }
        } else if event.type_tag() == &TypeTag::Struct(ToLBRExchangeRateUpdateEvent::struct_tag()) {
            let update_event = ToLBRExchangeRateUpdateEvent::try_from(&event)?;
            EventDataView::ToLBRExchangeRateUpdate {
                currency_code: update_event.currency_code().to_string(),
                new_to_lbr_exchange_rate: update_event.new_to_lbr_exchange_rate(),
            }
        } else if event.type_tag() == &TypeTag::Struct(MintEvent::struct_tag()) {
            let mint_event = MintEvent::try_from(&event)?;
            let amount_view =
                AmountView::new(mint_event.amount(), mint_event.currency_code().as_str());
            EventDataView::Mint {
                amount: amount_view,
            }
        } else if event.type_tag() == &TypeTag::Struct(ReceivedMintEvent::struct_tag()) {
            let received_mint_event = ReceivedMintEvent::try_from(&event)?;
            let amount_view = AmountView::new(
                received_mint_event.amount(),
                received_mint_event.currency_code().as_str(),
            );
            let destination_address =
                BytesView::from(received_mint_event.destination_address().as_ref());
            EventDataView::ReceivedMint {
                amount: amount_view,
                destination_address,
            }
        } else if event.type_tag() == &TypeTag::Struct(ComplianceKeyRotationEvent::struct_tag()) {
            let rotation_event = ComplianceKeyRotationEvent::try_from(&event)?;
            EventDataView::ComplianceKeyRotation {
                new_compliance_public_key: hex::encode(rotation_event.new_compliance_public_key()),
                time_rotated_seconds: rotation_event.time_rotated_seconds(),
            }
        } else if event.type_tag() == &TypeTag::Struct(BaseUrlRotationEvent::struct_tag()) {
            let rotation_event = BaseUrlRotationEvent::try_from(&event)?;
            String::from_utf8(rotation_event.new_base_url().to_vec())
                .map(|new_base_url| EventDataView::BaseUrlRotation {
                    new_base_url,
                    time_rotated_seconds: rotation_event.time_rotated_seconds(),
                })
                .map_err(|_| format_err!("Unable to parse BaseUrlRotationEvent"))?
        } else if event.type_tag() == &TypeTag::Struct(NewBlockEvent::struct_tag()) {
            let new_block_event = NewBlockEvent::try_from(&event)?;
            EventDataView::NewBlock {
                proposer: BytesView::from(new_block_event.proposer().as_ref()),
                round: new_block_event.round(),
                proposed_time: new_block_event.proposed_time(),
            }
        } else if event.type_tag() == &TypeTag::Struct(NewEpochEvent::struct_tag()) {
            let new_epoch_event = NewEpochEvent::try_from(&event)?;
            EventDataView::NewEpoch {
                epoch: new_epoch_event.epoch(),
            }
        } else if event.type_tag() == &TypeTag::Struct(CreateAccountEvent::struct_tag()) {
            let create_account_event = CreateAccountEvent::try_from(&event)?;
            let created_address = BytesView::from(create_account_event.created().as_ref());
            let role_id = create_account_event.role_id();
            EventDataView::CreateAccount {
                created_address,
                role_id,
            }
        } else if event.type_tag() == &TypeTag::Struct(UpgradeEvent::struct_tag()) {
            // TODO: missing test
            let upgrade_event = UpgradeEvent::try_from(&event)?;
            EventDataView::Upgrade {
                write_set: BytesView::from(upgrade_event.write_set()),
            }
        } else {
            EventDataView::Unknown {}
        };

        Ok(data)
    }
}

impl TryFrom<(u64, ContractEvent)> for EventView {
    type Error = Error;

    fn try_from((txn_version, event): (u64, ContractEvent)) -> Result<Self> {
        Ok(EventView {
            key: BytesView::from(event.key().as_bytes()),
            sequence_number: event.sequence_number(),
            transaction_version: txn_version,
            data: event.try_into()?,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MetadataView {
    pub version: u64,
    pub timestamp: u64,
    pub chain_id: u8,
    pub script_hash_allow_list: Option<Vec<BytesView>>,
    pub module_publishing_allowed: Option<bool>,
    pub libra_version: Option<u64>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct BytesView(pub String);

impl BytesView {
    pub fn into_bytes(self) -> Result<Vec<u8>, Error> {
        Ok(hex::decode(self.0)?)
    }
}

impl From<&[u8]> for BytesView {
    fn from(bytes: &[u8]) -> Self {
        Self(hex::encode(bytes))
    }
}

impl From<&Vec<u8>> for BytesView {
    fn from(bytes: &Vec<u8>) -> Self {
        Self(hex::encode(bytes))
    }
}

impl From<Vec<u8>> for BytesView {
    fn from(bytes: Vec<u8>) -> Self {
        Self(hex::encode(bytes))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MoveAbortExplanationView {
    category: String,
    category_description: String,
    reason: String,
    reason_description: String,
}

impl TryFrom<&KeptVMStatus> for MoveAbortExplanationView {
    type Error = ();
    fn try_from(status: &KeptVMStatus) -> Result<MoveAbortExplanationView, Self::Error> {
        match status {
            KeptVMStatus::MoveAbort(AbortLocation::Module(module_id), abort_code) => {
                let error_context = move_explain::get_explanation(module_id, *abort_code);
                error_context
                    .map(|context| MoveAbortExplanationView {
                        category: context.category.code_name,
                        category_description: context.category.code_description,
                        reason: context.reason.code_name,
                        reason_description: context.reason.code_description,
                    })
                    .ok_or_else(|| ())
            }
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum VMStatusView {
    #[serde(rename = "executed")]
    Executed,
    #[serde(rename = "out_of_gas")]
    OutOfGas,
    #[serde(rename = "move_abort")]
    MoveAbort {
        location: String,
        abort_code: u64,
        explanation: Option<MoveAbortExplanationView>,
    },
    #[serde(rename = "execution_failure")]
    ExecutionFailure {
        location: String,
        function_index: u16,
        code_offset: u16,
    },
    #[serde(rename = "miscellaneous_error")]
    MiscellaneousError,
}

impl From<&KeptVMStatus> for VMStatusView {
    fn from(status: &KeptVMStatus) -> Self {
        match status {
            KeptVMStatus::Executed => VMStatusView::Executed,
            KeptVMStatus::OutOfGas => VMStatusView::OutOfGas,
            KeptVMStatus::MoveAbort(loc, abort_code) => VMStatusView::MoveAbort {
                explanation: MoveAbortExplanationView::try_from(status).ok(),
                location: loc.to_string(),
                abort_code: *abort_code,
            },
            KeptVMStatus::ExecutionFailure {
                location,
                function,
                code_offset,
            } => VMStatusView::ExecutionFailure {
                location: location.to_string(),
                function_index: *function,
                code_offset: *code_offset,
            },
            KeptVMStatus::MiscellaneousError => VMStatusView::MiscellaneousError,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TransactionView {
    pub version: u64,
    pub transaction: TransactionDataView,
    pub hash: String,
    pub bytes: BytesView,
    pub events: Vec<EventView>,
    pub vm_status: VMStatusView,
    pub gas_used: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TransactionsWithProofsView {
    pub first_transaction_version: Option<u64>,
    pub serialized_transactions: Vec<BytesView>,
    pub ledger_info: BytesView,
    pub proofs: TransactionsProofsView,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TransactionsProofsView {
    pub ledger_info_to_transaction_infos_proof: BytesView,
    pub transaction_infos: BytesView,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum TransactionDataView {
    #[serde(rename = "blockmetadata")]
    BlockMetadata { timestamp_usecs: u64 },
    #[serde(rename = "writeset")]
    WriteSet {},
    #[serde(rename = "user")]
    UserTransaction {
        sender: String,
        signature_scheme: String,
        signature: String,
        public_key: String,
        sequence_number: u64,
        chain_id: u8,
        max_gas_amount: u64,
        gas_unit_price: u64,
        gas_currency: String,
        expiration_timestamp_secs: u64,
        script_hash: String,
        script_bytes: BytesView,
        script: ScriptView,
    },
    #[serde(rename = "unknown")]
    UnknownTransaction {},
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type")]
// TODO cover all script types
pub enum ScriptView {
    #[serde(rename = "peer_to_peer_transaction")]
    PeerToPeer {
        receiver: String,
        amount: u64,
        currency: String,
        metadata: BytesView,
        metadata_signature: BytesView,
    },
    #[serde(rename = "mint_transaction")]
    Mint {
        receiver: String,
        currency: String,
        auth_key_prefix: BytesView,
        amount: u64,
    },
    #[serde(rename = "unknown")]
    Unknown {},
}

impl ScriptView {
    // TODO cover all script types
}

impl From<Transaction> for TransactionDataView {
    fn from(tx: Transaction) -> Self {
        match tx {
            Transaction::BlockMetadata(t) => TransactionDataView::BlockMetadata {
                timestamp_usecs: t.timestamp_usec(),
            },
            Transaction::GenesisTransaction(_) => TransactionDataView::WriteSet {},
            Transaction::UserTransaction(t) => {
                let script_hash = match t.payload() {
                    TransactionPayload::Script(s) => HashValue::sha3_256_of(s.code()),
                    _ => HashValue::zero(),
                }
                .to_hex();

                let script_bytes: BytesView = match t.payload() {
                    TransactionPayload::Script(s) => lcs::to_bytes(s).unwrap_or_default(),
                    _ => vec![],
                }
                .into();

                TransactionDataView::UserTransaction {
                    sender: t.sender().to_string(),
                    signature_scheme: t.authenticator().scheme().to_string(),
                    signature: hex::encode(t.authenticator().signature_bytes()),
                    public_key: hex::encode(t.authenticator().public_key_bytes()),
                    sequence_number: t.sequence_number(),
                    chain_id: t.chain_id().id(),
                    max_gas_amount: t.max_gas_amount(),
                    gas_unit_price: t.gas_unit_price(),
                    gas_currency: t.gas_currency_code().to_string(),
                    expiration_timestamp_secs: t.expiration_timestamp_secs(),
                    script_hash,
                    script_bytes,
                    script: t.into_raw_transaction().into_payload().into(),
                }
            }
        }
    }
}

impl From<AccountRole> for AccountRoleView {
    fn from(role: AccountRole) -> Self {
        match role {
            AccountRole::Unknown => AccountRoleView::Unknown,
            AccountRole::ChildVASP(child_vasp) => AccountRoleView::ChildVASP {
                parent_vasp_address: BytesView::from(&child_vasp.parent_vasp_addr().to_vec()),
            },
            AccountRole::ParentVASP { vasp, credential } => AccountRoleView::ParentVASP {
                human_name: credential.human_name().to_string(),
                base_url: credential.base_url().to_string(),
                expiration_time: credential.expiration_date(),
                compliance_key: BytesView::from(credential.compliance_public_key()),
                num_children: vasp.num_children(),
                compliance_key_rotation_events_key: BytesView::from(
                    credential.compliance_key_rotation_events().key().as_bytes(),
                ),
                base_url_rotation_events_key: BytesView::from(
                    credential.base_url_rotation_events().key().as_bytes(),
                ),
            },
            AccountRole::DesignatedDealer {
                dd_credential,
                preburn_balances,
                designated_dealer,
            } => AccountRoleView::DesignatedDealer {
                human_name: dd_credential.human_name().to_string(),
                base_url: dd_credential.base_url().to_string(),
                expiration_time: dd_credential.expiration_date(),
                compliance_key: BytesView::from(dd_credential.compliance_public_key()),
                preburn_balances: preburn_balances
                    .iter()
                    .map(|(currency_code, balance)| {
                        AmountView::new(balance.coin(), &currency_code.as_str())
                    })
                    .collect(),
                received_mint_events_key: BytesView::from(
                    designated_dealer.received_mint_events().key().as_bytes(),
                ),
                compliance_key_rotation_events_key: BytesView::from(
                    dd_credential
                        .compliance_key_rotation_events()
                        .key()
                        .as_bytes(),
                ),
                base_url_rotation_events_key: BytesView::from(
                    dd_credential.base_url_rotation_events().key().as_bytes(),
                ),
            },
        }
    }
}

impl From<TransactionPayload> for ScriptView {
    fn from(value: TransactionPayload) -> Self {
        let empty_vec: Vec<TransactionArgument> = vec![];
        let empty_ty_vec: Vec<String> = vec![];
        let unknown_currency = "unknown_currency".to_string();

        let (code, args, ty_args) = match value {
            TransactionPayload::WriteSet(_) => ("genesis".to_string(), empty_vec, empty_ty_vec),
            TransactionPayload::Script(script) => (
                get_transaction_name(script.code()),
                script.args().to_vec(),
                script
                    .ty_args()
                    .iter()
                    .map(|type_tag| match type_tag {
                        TypeTag::Struct(StructTag { module, .. }) => module.to_string(),
                        tag => format!("{}", tag),
                    })
                    .collect(),
            ),
            TransactionPayload::Module(_) => {
                ("module publishing".to_string(), empty_vec, empty_ty_vec)
            }
        };

        let res = match code.as_str() {
            "peer_to_peer_with_metadata_transaction" => {
                if let [TransactionArgument::Address(receiver), TransactionArgument::U64(amount), TransactionArgument::U8Vector(metadata), TransactionArgument::U8Vector(metadata_signature)] =
                    &args[..]
                {
                    Ok(ScriptView::PeerToPeer {
                        receiver: receiver.to_string(),
                        amount: *amount,
                        currency: ty_args.get(0).unwrap_or(&unknown_currency).to_string(),
                        metadata: BytesView::from(metadata),
                        metadata_signature: BytesView::from(metadata_signature),
                    })
                } else {
                    Err(format_err!("Unable to parse PeerToPeer arguments"))
                }
            }
            "mint" => {
                if let [TransactionArgument::Address(receiver), TransactionArgument::U8Vector(auth_key_prefix), TransactionArgument::U64(amount)] =
                    &args[..]
                {
                    let currency = ty_args.get(0).unwrap_or(&unknown_currency).to_string();
                    Ok(ScriptView::Mint {
                        receiver: receiver.to_string(),
                        auth_key_prefix: BytesView::from(auth_key_prefix),
                        amount: *amount,
                        currency,
                    })
                } else {
                    Err(format_err!("Unable to parse PeerToPeer arguments"))
                }
            }
            _ => Err(format_err!("Unknown scripts")),
        };
        res.unwrap_or(ScriptView::Unknown {})
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CurrencyInfoView {
    pub code: String,
    pub scaling_factor: u64,
    pub fractional_part: u64,
    pub to_lbr_exchange_rate: f32,
    pub mint_events_key: BytesView,
    pub burn_events_key: BytesView,
    pub preburn_events_key: BytesView,
    pub cancel_burn_events_key: BytesView,
    pub exchange_rate_update_events_key: BytesView,
}

impl From<&CurrencyInfoResource> for CurrencyInfoView {
    fn from(info: &CurrencyInfoResource) -> CurrencyInfoView {
        CurrencyInfoView {
            code: info.currency_code().to_string(),
            scaling_factor: info.scaling_factor(),
            fractional_part: info.fractional_part(),
            to_lbr_exchange_rate: info.exchange_rate(),
            mint_events_key: BytesView::from(info.mint_events().key().as_bytes()),
            burn_events_key: BytesView::from(info.burn_events().key().as_bytes()),
            preburn_events_key: BytesView::from(info.preburn_events().key().as_bytes()),
            cancel_burn_events_key: BytesView::from(info.cancel_burn_events().key().as_bytes()),
            exchange_rate_update_events_key: BytesView::from(
                info.exchange_rate_update_events().key().as_bytes(),
            ),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StateProofView {
    pub ledger_info_with_signatures: BytesView,
    pub epoch_change_proof: BytesView,
    pub ledger_consistency_proof: BytesView,
}

impl
    TryFrom<(
        LedgerInfoWithSignatures,
        EpochChangeProof,
        AccumulatorConsistencyProof,
    )> for StateProofView
{
    type Error = Error;

    fn try_from(
        (ledger_info_with_signatures, epoch_change_proof, ledger_consistency_proof): (
            LedgerInfoWithSignatures,
            EpochChangeProof,
            AccumulatorConsistencyProof,
        ),
    ) -> Result<StateProofView, Self::Error> {
        Ok(StateProofView {
            ledger_info_with_signatures: BytesView::from(&lcs::to_bytes(
                &ledger_info_with_signatures,
            )?),
            epoch_change_proof: BytesView::from(&lcs::to_bytes(&epoch_change_proof)?),
            ledger_consistency_proof: BytesView::from(&lcs::to_bytes(&ledger_consistency_proof)?),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AccountStateWithProofView {
    pub version: u64,
    pub blob: Option<BytesView>,
    pub proof: AccountStateProofView,
}

impl TryFrom<AccountStateWithProof> for AccountStateWithProofView {
    type Error = Error;

    fn try_from(
        account_state_with_proof: AccountStateWithProof,
    ) -> Result<AccountStateWithProofView, Error> {
        let blob = if let Some(account_blob) = account_state_with_proof.blob {
            Some(BytesView::from(&lcs::to_bytes(&account_blob)?))
        } else {
            None
        };
        Ok(AccountStateWithProofView {
            version: account_state_with_proof.version,
            blob,
            proof: AccountStateProofView::try_from(account_state_with_proof.proof)?,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AccountStateProofView {
    pub ledger_info_to_transaction_info_proof: BytesView,
    pub transaction_info: BytesView,
    pub transaction_info_to_account_proof: BytesView,
}

impl TryFrom<AccountStateProof> for AccountStateProofView {
    type Error = Error;

    fn try_from(account_state_proof: AccountStateProof) -> Result<AccountStateProofView, Error> {
        Ok(AccountStateProofView {
            ledger_info_to_transaction_info_proof: BytesView::from(&lcs::to_bytes(
                account_state_proof
                    .transaction_info_with_proof()
                    .ledger_info_to_transaction_info_proof(),
            )?),
            transaction_info: BytesView::from(&lcs::to_bytes(
                account_state_proof
                    .transaction_info_with_proof()
                    .transaction_info(),
            )?),
            transaction_info_to_account_proof: BytesView::from(&lcs::to_bytes(
                account_state_proof.transaction_info_to_account_proof(),
            )?),
        })
    }
}
