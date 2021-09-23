###################################################
# Imports
###################################################
import asyncio
import prism_core
import pytest
import secrets
from terra_sdk.client.lcd import AsyncLCDClient
from terra_sdk.key.mnemonic import MnemonicKey
from terra_util import Account
###################################################
# Setup
###################################################
BOMBAY = False
if BOMBAY:
    key = MnemonicKey(
        mnemonic="lemon flavor goddess anger reflect option remove learn author learn damp often bullet ketchup cricket menu moment figure sugar donor load tongue stone tray"
    )
else:
    key = None
gas_prices = {
    "uluna": "0.15",
    "usdr": "0.1018",
    "uusd": "0.15",
    "ukrw": "178.05",
    "umnt": "431.6259",
    "ueur": "0.125",
    "ucny": "0.97",
    "ujpy": "16",
    "ugbp": "0.11",
    "uinr": "11",
    "ucad": "0.19",
    "uchf": "0.13",
    "uaud": "0.19",
    "usgd": "0.2",
}
###################################################
# Fixtures
###################################################
# Required to use fixture(scope=session)
# https://stackoverflow.com/questions/56236637/using-pytest-fixturescope-module-with-pytest-mark-asyncio
# https://github.com/pytest-dev/pytest-asyncio/issues/68
@pytest.fixture(scope='session')
def event_loop():
    loop = asyncio.get_event_loop()
    yield loop
    loop.close()

@pytest.fixture(scope='session')
async def account():
    async with AsyncLCDClient(
        "http://localhost:1317", "localterra", gas_prices=gas_prices
    ) as lcd_client:
        async with Account(lcd_client=lcd_client, key=key) as account:
            print(f'Using Chain ID "{account.chain_id()}"...')
            yield account

@pytest.fixture(scope='session')
async def setup_contracts(account):
    """Sets up the PRISM contracts to prepare for testing."""
    contracts = await prism_core.setup_contracts(account)
    return contracts
###################################################
# Test Logic
###################################################
@pytest.mark.asyncio
@pytest.mark.dependency()
async def test_cluna_bonding(setup_contracts, account):
    """Tests cLuna bonding logic by randomly adding different amounts."""
    generator = secrets.SystemRandom()
    rand = []
    for _ in range(5):
        rnd = generator.randint(1, 10000000)
        await prism_core.bond_cluna(account, setup_contracts['prism_vault'], str(rnd))
        rand.append(rnd)
    bal = await setup_contracts['cluna_token'].query.balance(address=account.acc_address)
    assert bal['balance'] == str(sum(rand))

@pytest.mark.asyncio
@pytest.mark.dependency(depends=["test_cluna_bonding"])
async def test_cluna_splitting(setup_contracts, account):
    """Tests cLuna splitting (cLuna -> yLuna & pLuna) logic.
    
    Depends on cLuna being bonded already (e.g. "test_cluna_bonding").
    """
    # Step 1: Spit current cLuna balance and test...
    bal = await setup_contracts['cluna_token'].query.balance(address=account.acc_address)
    await prism_core.split_cluna(
        account,
        setup_contracts['cluna_token'],
        setup_contracts['prism_vault'],
        bal['balance']
    )
    yluna = await setup_contracts['yluna_token'].query.balance(address=account.acc_address)
    pluna = await setup_contracts['pluna_token'].query.balance(address=account.acc_address)
    assert int(yluna['balance']) + int(pluna['balance']) == int(bal['balance']) * 2
    # Step 2: Split additional cLuna and test balance...
    await prism_core.bond_cluna(account, setup_contracts['prism_vault'], bal['balance'])
    await prism_core.split_cluna(
        account,
        setup_contracts['cluna_token'],
        setup_contracts['prism_vault'],
        bal['balance']
    )
    yluna = await setup_contracts['yluna_token'].query.balance(address=account.acc_address)
    pluna = await setup_contracts['pluna_token'].query.balance(address=account.acc_address)
    assert int(yluna['balance']) + int(pluna['balance']) == int(bal['balance']) * 4
    # Step 3: Ensure cLuna balance is zero...
    bal = await setup_contracts['cluna_token'].query.balance(address=account.acc_address)
    assert '0' == bal['balance']
    # Step 4: Attempt to split cLuna without cLuna balance (should fail)...
    with pytest.raises(Exception) as ex:
        await prism_core.split_cluna(
            account,
            setup_contracts['cluna_token'],
            setup_contracts['prism_vault'],
            '1'
        )
    assert "execute wasm contract failed" in str(ex.value)

@pytest.mark.asyncio
@pytest.mark.dependency(depends=["test_cluna_splitting"])
async def test_yluna_staking(setup_contracts, account):
    # Step 1: Stake all yLuna into the Vault...
    yluna = await setup_contracts['yluna_token'].query.balance(address=account.acc_address)
    original_bal = yluna['balance']
    print(f"Attempting to stake {original_bal} yLuna...")
    await prism_core.stake_yluna(account, setup_contracts['yluna_token'], setup_contracts['yluna_staking'], original_bal)
    # Step 2: Update Rewards...
    await prism_core.update_global_index(setup_contracts['prism_vault'])
    # Step 3: Attempt to unstake 1 micro-yLuna
    await prism_core.unstake_yluna(setup_contracts['yluna_staking'], '1')
    # Step 4: Ensure that amount staked is 1 micro-yLuna less than before 
    s = await setup_contracts['yluna_staking'].query.reward_info(staker_addr=account.acc_address)
    print(f"{s}")
    assert s['staked_amt'] == str(int(original_bal) - 1)
    # Step 5: Ensure that yLuna balance is 1 micro-yLuna
    yluna = await setup_contracts['yluna_token'].query.balance(address=account.acc_address)
    assert yluna['balance'] == '1'
    # Step 6: Withdraw remaining bonded yLuna...
    await prism_core.unstake_yluna(setup_contracts['yluna_staking'], str(int(original_bal) - 1))
    # Step 7: Ensure that amount staked is 0
    s = await setup_contracts['yluna_staking'].query.reward_info(staker_addr=account.acc_address)
    print(f"{s}")
    assert s['staked_amt'] == '0'
    # Step 8: Ensure that yLuna balance is original yLuna amount
    yluna = await setup_contracts['yluna_token'].query.balance(address=account.acc_address)
    assert yluna['balance'] == original_bal
    # Step 9: Withdraw all rewards
    await prism_core.withdraw_all_rewards(account, setup_contracts['prism_vault'], setup_contracts['yluna_staking'])
    # Step 10: Ensure that the yLuna balance is higher than the original yLuna amount
    yluna = await setup_contracts['yluna_token'].query.balance(address=account.acc_address)
    print(f"Total yLuna balance: {yluna['balance']}")
    assert int(yluna['balance']) > int(original_bal)
    # Step 11: Ensure that there are no rewards available in the Vault
    s = await setup_contracts['yluna_staking'].query.reward_info(staker_addr=account.acc_address)
    print(f"{s}")
    for ss in s['reward_infos']:
        assert ss['amount'] == '0'

@pytest.mark.asyncio
@pytest.mark.dependency(depends=["test_yluna_staking"])
async def test_cluna_merging(setup_contracts, account):
    yluna = await setup_contracts['yluna_token'].query.balance(address=account.acc_address)
    original_bal = yluna['balance']
    await prism_core.merge_cluna(
        account,
        setup_contracts['yluna_token'],
        setup_contracts['pluna_token'],
        setup_contracts['prism_vault'],
        original_bal
    )
    yluna = await setup_contracts['yluna_token'].query.balance(address=account.acc_address)
    assert yluna['balance'] == '0'
    cluna = await setup_contracts['cluna_token'].query.balance(address=account.acc_address)
    print(f"Total cLuna balance: {cluna['balance']}")
    assert cluna['balance'] == original_bal
