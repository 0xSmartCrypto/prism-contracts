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
    return await prism_core.setup_contracts(account)
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
    """Tests cLuna splitting (cLuna -> yLuna & pLuna) logic."""
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

# TODO: test invalid values...
