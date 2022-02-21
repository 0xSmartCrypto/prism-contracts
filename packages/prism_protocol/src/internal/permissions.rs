use cosmwasm_std::{Addr, MessageInfo, StdError, StdResult};

pub fn check_sender(info: &MessageInfo, must_be: &Addr) -> StdResult<()> {
    if info.sender == *must_be {
        return Ok(());
    }
    Err(StdError::generic_err("unauthorized"))
}

#[test]
fn test_check_sender() {
    let info = MessageInfo {
        sender: Addr::unchecked("monkey"),
        funds: vec![],
    };
    let sender = Addr::unchecked("monkey");
    assert_eq!(check_sender(&info, &sender), Ok(()));

    let info = MessageInfo {
        sender: Addr::unchecked("monkey"),
        funds: vec![],
    };
    let sender = Addr::unchecked("gorilla");
    assert_eq!(
        check_sender(&info, &sender),
        Err(StdError::generic_err("unauthorized"))
    );
}
