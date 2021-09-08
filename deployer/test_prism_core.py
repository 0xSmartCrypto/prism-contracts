import asyncio
from terra_util import Account, Asset
from terra_sdk.key.mnemonic import MnemonicKey

BOMBAY = True


async def test():
    if BOMBAY:
        key = MnemonicKey(
            mnemonic="lemon flavor goddess anger reflect option remove learn author learn damp often bullet ketchup cricket menu moment figure sugar donor load tongue stone tray"
        )
    else:
        key = None
    account = Account(bombay=BOMBAY, key=key)
    print(account.acc_address)

    code_ids = await account.store_contracts()
    terraswap_factory = await account.contract.create(
        code_ids["terraswap_factory"],
        pair_code_id=int(code_ids["terraswap_pair"]),
        token_code_id=int(code_ids["cw20_base"]),
    )

    prism_token = await account.contract.create(
        code_ids["cw20_base"],
        name="Prism Token",
        symbol="PRISM",
        decimals=6,
        initial_balances=[
            {"address": account.acc_address, "amount": "10000000"},
        ],
        mint=None,
    )

    prism_pair = account.contract(
        (
            await terraswap_factory.create_pair(
                asset_infos=[
                    Asset.cw20_asset_info(prism_token),
                    Asset.native_asset_info("uusd"),
                ]
            )
        )
        .logs[0]
        .events_by_type["from_contract"]["pair_contract_addr"][0]
    )

    await prism_token.increase_allowance(amount="10000", spender=prism_pair)

    await prism_pair.provide_liquidity(
        assets=[
            Asset.asset(prism_token, amount="10000"),
            Asset.asset("uusd", amount="10000", native=True),
        ],
        _send={"uusd": "10000"},
    )

    prism_vault = await account.contract.create(
        code_ids["prism_vault"],
        epoch_period=10,
        underlying_coin_denom="uluna",
        unbonding_period=10,
        peg_recovery_fee="0.005",
        er_threshold="0.01",
        validator="terravaloper1krj7amhhagjnyg2tkkuh6l0550y733jnjnnlzy"
        if account.bombay
        else "terravaloper1dcegyrekltswvyy0xy69ydgxn9x8x32zdy3ua5",
        init_coins={"uluna": "1000000"},
    )

    print(prism_vault.address)

    cluna_token = await account.contract.create(
        code_ids["cw20_base"],
        name="cLUNA token",
        symbol="CLUNA",
        decimals=6,
        initial_balances=[],
        mint={"minter": prism_vault},
    )
    yluna_token = await account.contract.create(
        code_ids["cw20_base"],
        name="yLUNA token",
        symbol="YLUNA",
        decimals=6,
        initial_balances=[],
        mint={"minter": prism_vault},
    )
    pluna_token = await account.contract.create(
        code_ids["cw20_base"],
        name="pLUNA token",
        symbol="PLUNA",
        decimals=6,
        initial_balances=[],
        mint={"minter": prism_vault},
    )

    yluna_staking = await account.contract.create(
        code_ids["prism_yasset_staking"],
        vault=prism_vault,
        prism_token=prism_token,
        yluna_token=yluna_token,
        reward_denom="uusd",
        prism_pair=prism_pair,
    )

    await prism_vault.update_config(
        yluna_staking=yluna_staking,
        cluna_contract=cluna_token,
        yluna_contract=yluna_token,
        pluna_contract=pluna_token,
    )

    await prism_vault.bond(
        validator="terravaloper1krj7amhhagjnyg2tkkuh6l0550y733jnjnnlzy"
        if account.bombay
        else "terravaloper1dcegyrekltswvyy0xy69ydgxn9x8x32zdy3ua5",
        _send={"uluna": "1000000"},
    )

    print(await cluna_token.query.balance(address=account.acc_address))

    await account.chain(
        cluna_token.increase_allowance(spender=prism_vault, amount="1000000"),
        prism_vault.split(amount="1000000"),
    )
    print(await yluna_token.query.balance(address=account.acc_address))
    print(await pluna_token.query.balance(address=account.acc_address))

    await yluna_token.send(
        amount="1000000", contract=yluna_staking, msg=yluna_staking.bond()
    )
    # await prism_vault.update_global_index()
    #
    # resp = await yluna_staking.withdraw()
    import pprint

    # pprint.pprint(resp.logs[0].events_by_type)
    await yluna_staking.unbond(amount="1000000")

    await account.chain(
        yluna_token.increase_allowance(spender=prism_vault, amount="1000000"),
        pluna_token.increase_allowance(spender=prism_vault, amount="1000000"),
        prism_vault.merge(amount="1000000"),
    )

    print(await cluna_token.query.balance(address=account.acc_address))
    print(await prism_token.query.balance(address=account.acc_address))
    print(prism_vault.address)
    print(await prism_vault.query.config())

    # await cluna_token.send(
    #     amount="1000000",
    #     contract=prism_vault,
    #     msg=prism_vault.unbond(),
    # )
    # await prism_vault.withdraw_unbonded()


if __name__ == "__main__":
    asyncio.get_event_loop().run_until_complete(test())
