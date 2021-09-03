import asyncio
from terra_util import Account, Asset


async def test():
    account = Account()
    code_ids = await account.store_contracts()

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

    prism_vault = await account.contract.create(
        code_ids["prism_vault"],
        epoch_period=10,
        underlying_coin_denom="uluna",
        unbonding_period=10,
        peg_recovery_fee="0.005",
        er_threshold="0.01",
        validator="terravaloper1dcegyrekltswvyy0xy69ydgxn9x8x32zdy3ua5",
        init_coins={"uluna": "1000000"}
    )

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
        prism_pair=prism_token # placeholder for now
    )

    await prism_vault.update_config(
        cluna_contract=cluna_token,
        yluna_contract=yluna_token,
        pluna_contract=pluna_token
    )

    await prism_vault.bond(
        validator="terravaloper1dcegyrekltswvyy0xy69ydgxn9x8x32zdy3ua5",
        _send={"uluna": "1000000"}
    )

    print(await cluna_token.query.balance(address=account.acc_address))

    await account.chain(
        cluna_token.increase_allowance(spender=prism_vault, amount="1000000"),
        prism_vault.split(amount="1000000")
    )
    print(await yluna_token.query.balance(address=account.acc_address))
    print(await pluna_token.query.balance(address=account.acc_address))

    await account.chain(
        yluna_token.increase_allowance(spender=prism_vault, amount="1000000"),
        pluna_token.increase_allowance(spender=prism_vault, amount="1000000"),
        prism_vault.merge(amount="1000000")
    )

    print(await cluna_token.query.balance(address=account.acc_address))



if __name__ == '__main__':
    asyncio.get_event_loop().run_until_complete(test())