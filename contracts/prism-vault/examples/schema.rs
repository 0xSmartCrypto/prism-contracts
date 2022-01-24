use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};
use prism_protocol::vault::{
    AllHistoryResponse, ConfigResponse, CurrentBatchResponse, Cw20HookMsg, ExecuteMsg,
    InstantiateMsg, QueryMsg, StateResponse, UnbondHistory, WhitelistedValidatorsResponse,
    WithdrawableUnbondedResponse,
};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(AllHistoryResponse), &out_dir);
    export_schema(&schema_for!(ConfigResponse), &out_dir);
    export_schema(&schema_for!(CurrentBatchResponse), &out_dir);
    export_schema(&schema_for!(Cw20HookMsg), &out_dir);
    export_schema(&schema_for!(StateResponse), &out_dir);
    export_schema(&schema_for!(UnbondHistory), &out_dir);
    export_schema(&schema_for!(WhitelistedValidatorsResponse), &out_dir);
    export_schema(&schema_for!(WithdrawableUnbondedResponse), &out_dir);
}
