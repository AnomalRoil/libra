## Method get_transactions_with_proofs

**Description**

Get the transactions on the blockchain along with the proofs necessary to verify the said transactions.


### Parameters

| Name           | Type           | Description                                                          |
|----------------|----------------|----------------------------------------------------------------------|
| start_version  | unsigned int64 | Start on this transaction version for this query                     |
| limit          | unsigned int64 | Limit the number of transactions returned, the max value is 1000     |

### Returns



| Name                      | Type               | Description                   |
|---------------------------|--------------------|-------------------------------|
| first_transaction_version | unsigned int64     | The version of the first transactions in the returned data |
| serialized_transactions   | List<string>       | An array of hex encoded strings with the raw bytes of the returned `Transaction` |
| ledger_info               | string             | An hex encoded string of raw bytes of the `LedgerInfoWithSignature` at the moment of the returned proofs |
| proofs                    | TransactionsProofs | The proofs, see below.   |


The proofs:


| Name           | Type           | Description                                                          |
|----------------|----------------|----------------------------------------------------------------------|
| ledger_info_to_transaction_infos_proof  | string | An hex encoded string of raw bytes of a `Vec<AccumulatorRangeProof<TransactionAccumulatorHasher>>` that contains the proofs of the returned transactions |
| transaction_infos          | string | An hex encoded string of raw bytes of a `Vec<TransactionInfo>` that corresponds to returned transcations    |

