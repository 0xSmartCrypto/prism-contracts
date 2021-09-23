from terra_util import Account, Asset, store_contracts

DEFAULT_POLL_ID = 1
DEFAULT_QUORUM = "0.3"
DEFAULT_THRESHOLD = "0.5"
DEFAULT_VOTING_PERIOD = 2
DEFAULT_EFFECTIVE_DELAY = 2
DEFAULT_PROPOSAL_DEPOSIT = "10000000000"
DEFAULT_SNAPSHOT_PERIOD = 0
DEFAULT_VOTER_WEIGHT = "0.1"

async def setup_contracts(account):
    print(f'Using account address: {account.acc_address}')

    if "localterra" in account.chain_id():
        code_ids = await store_contracts(
            accounts=[Account(key=f"test{i}") for i in range(1, 11)]
        )
    else:
        code_ids = await store_contracts(accounts=[account])

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

    xprism_token = await account.contract.create(
        code_ids["cw20_base"],
        name="xPrism Token",
        symbol="xPRISM",
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
        er_threshold="1.00",
        validator="terravaloper1dcegyrekltswvyy0xy69ydgxn9x8x32zdy3ua5"
        if "localterra" in account.chain_id()
        else "terravaloper1krj7amhhagjnyg2tkkuh6l0550y733jnjnnlzy",
        init_coins={"uluna": "1000000"},
    )
    print(f'Prism Vault Address: {prism_vault.address}')

    cluna_token = await account.contract.create(
        code_ids["cw20_base"],
        name="cLUNA token",
        symbol="CLUNA",
        decimals=6,
        initial_balances=[
            # It's important to add the same luna here as the prism_vault contract is instantiated with
            # or else very bad things will happen (exchange rate miscalculation)
            {"address": prism_vault, "amount": "1000000"}
        ],
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

    prism_gov = await account.contract.create(
        code_ids["prism_gov"],
        prism_token=prism_token,
        xprism_token=xprism_token,
        quorum=DEFAULT_QUORUM,
        threshold=DEFAULT_THRESHOLD,
        voting_period=DEFAULT_VOTING_PERIOD,
        effective_delay=DEFAULT_EFFECTIVE_DELAY,
        proposal_deposit=DEFAULT_PROPOSAL_DEPOSIT,
        voter_weight=DEFAULT_VOTER_WEIGHT,
        snapshot_period=DEFAULT_SNAPSHOT_PERIOD,
    )

    prism_collector = await account.contract.create(
        code_ids["prism_collector"],
        distribution_contract=prism_gov,
        terraswap_factory=terraswap_factory,
        prism_token=prism_token,
        base_denom="uusd",
        owner=account.acc_address,
    )

    yluna_staking = await account.contract.create(
        code_ids["prism_yasset_staking"],
        vault=prism_vault,
        gov=prism_gov,
        collector=prism_collector,
        yluna_token=yluna_token,
        pluna_token=pluna_token,
        cluna_token=cluna_token,
        init_coins={"uusd": "1000"},
    )

    await prism_vault.update_config(
        yluna_staking=yluna_staking,
        cluna_contract=cluna_token,
        yluna_contract=yluna_token,
        pluna_contract=pluna_token,
    )

    return {
        "account_address": account.acc_address,
        "prism_token": prism_token,
        "xprism_token": xprism_token,
        "prism_pair": prism_pair,
        "prism_vault": prism_vault,
        "cluna_token": cluna_token,
        "yluna_token": yluna_token,
        "pluna_token": pluna_token,
        "prism_gov": prism_gov,
        "prism_collector": prism_collector,
        "yluna_staking": yluna_staking
    }

    print('Bonding...')
    bond1 = await prism_vault.bond(
        validator="terravaloper1krj7amhhagjnyg2tkkuh6l0550y733jnjnnlzy"
        if account.bombay
        else "terravaloper1dcegyrekltswvyy0xy69ydgxn9x8x32zdy3ua5",
        _send={"uluna": "1000000"},
    )

    bond2 = await prism_vault.bond(
        validator="terravaloper1krj7amhhagjnyg2tkkuh6l0550y733jnjnnlzy"
        if account.bombay
        else "terravaloper1dcegyrekltswvyy0xy69ydgxn9x8x32zdy3ua5",
        _send={"uluna": "1000000"},
    )

    print(f'Bond #1: {bond1.txhash}')
    print(f'Bond #2: {bond2.txhash}')

    print(f'cLuna bal: {await cluna_token.query.balance(address=account.acc_address)}')

    print('Splitting...')
    await account.chain(
        cluna_token.increase_allowance(spender=prism_vault, amount="1000000"),
        prism_vault.split(amount="1000000"),
    )
    print(f'yLuna bal: {await yluna_token.query.balance(address=account.acc_address)}')
    print(f'pLuna bal: {await pluna_token.query.balance(address=account.acc_address)}')
    print(f'cLuna bal: {await cluna_token.query.balance(address=account.acc_address)}')
    return

    await yluna_token.send(
        amount="1000000", contract=yluna_staking, msg=yluna_staking.bond()
    )

    resp = await prism_vault.update_global_index()
    import pprint
    for log in resp.logs:
        pprint.pprint(log.events_by_type)

    resp = await yluna_staking.withdraw()
    print(await yluna_token.query.balance(address=account.acc_address))

    import pprint
    for log in resp.logs:
        pprint.pprint(log.events_by_type)

    await yluna_staking.unbond(amount="1000000")

    await account.chain(
        yluna_token.increase_allowance(spender=prism_vault, amount="1000000"),
        pluna_token.increase_allowance(spender=prism_vault, amount="1000000"),
        prism_vault.merge(amount="1000000"),
    )

    print(await cluna_token.query.balance(address=account.acc_address))
    print(await prism_token.query.balance(address=account.acc_address))
    print(await prism_token.query.balance(address=prism_gov))
    print(prism_vault.address)
    print(await prism_vault.query.config())

    # await cluna_token.send(
    #     amount="1000000",
    #     contract=prism_vault,
    #     msg=prism_vault.unbond(),
    # )
    # await prism_vault.withdraw_unbonded()

    print('All done.')

async def bond_cluna(account, prism_vault, amount):
    await prism_vault.bond(
        validator="terravaloper1dcegyrekltswvyy0xy69ydgxn9x8x32zdy3ua5"
        if "localterra" in account.chain_id()
        else "terravaloper1krj7amhhagjnyg2tkkuh6l0550y733jnjnnlzy",
        _send={"uluna": amount},
    )

async def split_cluna(account, cluna_token, prism_vault, amount):
    await account.chain(
        cluna_token.increase_allowance(spender=prism_vault, amount=amount),
        prism_vault.split(amount=amount),
    )

async def stake_yluna(account, yluna_token, yluna_staking, amount):
    await yluna_token.send(
        amount=amount, contract=yluna_staking, msg=yluna_staking.bond()
    )

async def update_global_index(prism_vault):
    resp = await prism_vault.update_global_index()
    #import pprint
    #for log in resp.logs:
    #    pprint.pprint(log.events_by_type)
    return resp

async def unstake_yluna(yluna_staking, amount):
    await yluna_staking.unbond(amount=amount)

async def withdraw_all_rewards(account, prism_vault, yluna_staking):
    await account.chain(
        prism_vault.update_global_index(),
        yluna_staking.withdraw()
    )

async def merge_cluna(account, yluna_token, pluna_token, prism_vault, amount):
    await account.chain(
        yluna_token.increase_allowance(spender=prism_vault, amount=amount),
        pluna_token.increase_allowance(spender=prism_vault, amount=amount),
        prism_vault.merge(amount=amount),
    )
