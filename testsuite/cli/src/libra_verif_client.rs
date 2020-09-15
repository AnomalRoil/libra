// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

pub struct LibraVerifClient {
    client: JsonRpcClient,
    /// The latest verified chain state.
    trusted_state: TrustedState,
    /// The most recent epoch change ledger info. This is `None` if we only know
    /// about our local [`Waypoint`] and have not yet ratcheted to the remote's
    /// latest state.
    latest_epoch_change_li: Option<LedgerInfoWithSignatures>,
}

impl LibraVerifClient {
    /// Construct a new Client instance.
    pub fn new(url: Url, waypoint: Waypoint) -> Result<Self> {
        let initial_trusted_state = TrustedState::from(waypoint);
        let client = JsonRpcClient::new(url)?;
        Ok(LibraVerifClient {
            client,
            trusted_state: initial_trusted_state,
            latest_epoch_change_li: None,
        })
    }
}