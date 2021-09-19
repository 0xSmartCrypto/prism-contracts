import asyncio
from terra_sdk.client.localterra import AsyncLocalTerra
from terra_sdk.core.wasm import (
    MsgStoreCode,
    MsgInstantiateContract,
    MsgExecuteContract,
)
from terra_sdk.core.bank import MsgSend
from terra_sdk.util.contract import get_code_id, read_file_as_b64
import os
import json
import base64

CONTRACT_DIR = os.path.join(
    os.path.dirname(os.path.dirname(__file__)),
    "artifacts",
)

class Asset:
    @staticmethod
    def cw20_asset_info(haddr):
        return {"token": {"contract_addr": haddr}}

    @staticmethod
    def native_asset_info(denom):
        return {"native_token": {"denom": denom}}

    @staticmethod
    def nft_asset_info(contract_addr, token_id):
        return {"nft": {"contract_addr": contract_addr, "token_id": token_id}}

    @staticmethod
    def asset(string, amount, native=False):
        if not native:
            return {"info": Asset.cw20_asset_info(string), "amount": amount}
        else:
            return {"info": Asset.native_asset_info(string), "amount": amount}

    @staticmethod
    def nft(contract_addr, token_id):
        return {
            "info": Asset.nft_asset_info(contract_addr, token_id),
            "amount": "1",
        }


def custom_objs_to_json(obj):
    if type(obj) == dict:
        return {k: custom_objs_to_json(v) for k, v in obj.items()}
    if type(obj) in {list, tuple}:
        return [custom_objs_to_json(i) for i in obj]
    # contract objects
    if hasattr(obj, "address"):
        return obj.address
    # executemessage objects
    if hasattr(obj, "json"):
        return base64.b64encode(bytes(json.dumps(obj.json), "ascii")).decode()
    return obj


async def store_contracts(accounts):

    contract_names = [i[:-5] for i in os.listdir(CONTRACT_DIR) if i.endswith(".wasm")]

    res = {}

    async def store_loop(account):
        while contract_names:
            name = contract_names.pop()
            contract_id = await account.store_contract(name)
            res[name] = contract_id

    await asyncio.gather(*[store_loop(i) for i in accounts])
    return res

lt = AsyncLocalTerra(gas_prices={"uusd": "0.15"})

class Account:
    def __init__(self, lcd_client=None, key=None, ecosystem=None):
        self.ecosystem = ecosystem
        if lcd_client:

            self.terra = lcd_client
        else:
            self.terra = lt

        if isinstance(key, str):
            key = lt.wallets[key].key
        if key is None:
            key = lt.wallets["test1"].key

        self.deployer = self.terra.wallet(key)
        self.key = self.deployer.key

        self.address = self.acc_address = self.key.acc_address
        self.sequence = None

        outer_obj = self

        class Message:
            def __init__(self):
                self.msg = None

            def __await__(self):
                return outer_obj.sign_and_broadcast(self.msg).__await__()

        class ExecuteMessage(Message):
            def __init__(self, contract, json, send=None):
                super(ExecuteMessage, self).__init__()
                self.contract = contract
                self.json = custom_objs_to_json(json)
                self.msg = MsgExecuteContract(
                    outer_obj.acc_address, self.contract.address, self.json, send
                )

        class InstantiateMessage(Message):
            def __init__(self, code_id, json, init_coins=None):
                super(InstantiateMessage, self).__init__()
                self.json = custom_objs_to_json(json)
                self.msg = MsgInstantiateContract(
                    outer_obj.acc_address,
                    outer_obj.acc_address,
                    code_id,
                    self.json,
                    init_coins=init_coins,
                )

        class SendMsg(Message):
            def __init__(self, recipient, amount):
                super(SendMsg, self).__init__()
                self.msg = MsgSend(
                    amount=amount,
                    to_address=recipient,
                    from_address=outer_obj.acc_address,
                )

        class Contract:
            def __init__(self, address):
                self.address = address

            @classmethod
            async def create(cls, code_id, init_coins=None, **kwargs):
                msg = InstantiateMessage(code_id, kwargs, init_coins=init_coins)
                result = await msg
                if result.logs:
                    contract_address = result.logs[0].events_by_type[
                        "instantiate_contract"
                    ]["contract_address"][-1]
                    return cls(contract_address)
                else:
                    raise ValueError("could not parse code id -- tx logs are empty.")

            @property
            def query(self):
                contract_addr = self.address

                class ContractQuerier:
                    def __getattr__(self, item):
                        async def result_fxn(**kwargs):
                            kwargs = custom_objs_to_json(kwargs)
                            return await outer_obj.terra.wasm.contract_query(
                                contract_addr, custom_objs_to_json({item: kwargs})
                            )

                        return result_fxn

                return ContractQuerier()

            def __getattr__(self, item):
                def result_fxn(_send=None, **kwargs):
                    return ExecuteMessage(
                        contract=self, json={item: kwargs}, send=_send
                    )

                return result_fxn

        self.send = SendMsg
        self.execute = ExecuteMessage
        self.contract = Contract

    async def store_contract(self, contract_name):

        contract_bytes = read_file_as_b64(f"{CONTRACT_DIR}/{contract_name}.wasm")
        store_code = MsgStoreCode(self.acc_address, contract_bytes)

        result = await self.sign_and_broadcast(store_code)
        code_id = get_code_id(result)
        print(f"Code id for {contract_name} is {code_id}")
        return code_id

    async def chain(self, *messages):
        return await self.sign_and_broadcast(*[i.msg for i in messages])

    def chain_id(self):
        return self.terra.chain_id

    async def sign_and_broadcast(self, *msgs):
        if self.sequence is None:
            self.sequence = await self.deployer.sequence()

        try:
            tx = await self.deployer.create_and_sign_tx(
                msgs=msgs,
                gas_prices={"uusd": "0.15"},
                gas_adjustment=1.5,
                sequence=self.sequence,
            )
            result = await self.terra.tx.broadcast(tx)
            self.sequence += 1
            if result.is_tx_error():
                raise Exception(result.raw_log)
            return result
        except:
            self.sequence = await self.deployer.sequence()
            raise

    def __getattr__(self, item):
        if self.ecosystem is None:
            raise AttributeError
        return self.contract(self.ecosystem.contract_addrs[item])

    async def __aenter__(self):
        await self.terra.__aenter__()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.terra.__aexit__(exc_type, exc_val, exc_tb)
