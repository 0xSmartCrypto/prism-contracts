use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};
use prism_protocol::gov::{
    Cw20HookMsg, ExecuteMsg, InstantiateMsg, PollResponse, PollsResponse,
    PrismWithdrawOrdersResponse, QueryMsg, VotersResponse, VotersResponseItem,
    VotingTokensResponse,
};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(Cw20HookMsg), &out_dir);
    export_schema(&schema_for!(PollResponse), &out_dir);
    export_schema(&schema_for!(PollsResponse), &out_dir);
    export_schema(&schema_for!(VotingTokensResponse), &out_dir);
    export_schema(&schema_for!(VotersResponse), &out_dir);
    export_schema(&schema_for!(VotersResponseItem), &out_dir);
    export_schema(&schema_for!(PrismWithdrawOrdersResponse), &out_dir);
}
