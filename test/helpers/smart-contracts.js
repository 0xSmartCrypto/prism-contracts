//////////////////////////////////////////////////////////////////////
// prism-contracts - refracting defi
//////////////////////////////////////////////////////////////////////
const {
    // eslint-disable-next-line no-unused-vars
    LCDClient, MsgInstantiateContract, MsgStoreCode, Wallet
} = require('@terra-money/terra.js')
const fs = require('fs')
const path = require('path')
//////////////////////////////////////////////////////////////////////
// helpers
//////////////////////////////////////////////////////////////////////
function remap_events(arr) {
    return Object.fromEntries(arr.map((i) => [i.type, i.attributes]))
}
function remap_attributes(arr) {
    return Object.fromEntries(arr.map((i) => [i.key, i.value]))
}
//////////////////////////////////////////////////////////////////////
// logic
//////////////////////////////////////////////////////////////////////
async function upload_prism_casset_token_contract(lcd, wallet) {
    const storeCode = new MsgStoreCode(
        wallet.key.accAddress,
        fs.readFileSync(path.join(__dirname, '..', '..', 'prism-casset-token', 'artifacts', 'prism_casset_token.wasm')).toString('base64')
    )
    const storeCodeTx = await wallet.createAndSignTx({
        msgs: [storeCode]
    })

    const storeCodeTxResult = await lcd.tx.broadcast(storeCodeTx)
    return remap_attributes(remap_events(storeCodeTxResult.logs[0].events).store_code).code_id
}
async function upload_prism_luna_hub_contract(lcd, wallet) {
    const storeCode = new MsgStoreCode(
        wallet.key.accAddress,
        fs.readFileSync(path.join(__dirname, '..', '..', 'prism-luna-contracts', 'prism-luna-hub', 'artifacts', 'prism_luna_hub.wasm')).toString('base64')
    )
    const storeCodeTx = await wallet.createAndSignTx({
        msgs: [storeCode]
    })

    const storeCodeTxResult = await lcd.tx.broadcast(storeCodeTx)
    return remap_attributes(remap_events(storeCodeTxResult.logs[0].events).store_code).code_id
}
/**
 * @param {LCDClient} lcd
 * @param {Wallet} wallet
 */
async function upload_all_contracts(lcd, wallet) {
    const casset = await upload_prism_casset_token_contract(lcd, wallet)
    const luna_hub = await upload_prism_luna_hub_contract(lcd, wallet)
    return {
        casset_token_contract: casset,
        prism_luna_hub: luna_hub
    }
}
/**
 * @param {LCDClient} lcd
 * @param {Wallet} wallet
 */
async function instantiate_prism_luna_hub_contract(lcd, wallet, code_id) {
    const instantiateMsg = new MsgInstantiateContract(
        wallet.key.accAddress,
        wallet.key.accAddress,
        Number(code_id),
        {}
    )
    const instantiateMsgTx = await wallet.createAndSignTx({
        msgs: [instantiateMsg]
    })

    const instantiateResult = await lcd.tx.broadcast(instantiateMsgTx)
    return remap_attributes(
        remap_events(instantiateResult.logs[0].events).instantiate_contract
    ).contract_address
}
/**
 * @param {LCDClient} lcd
 * @param {Wallet} wallet
 */
 async function instantiate_casset_token_contract(lcd, wallet, code_id, name, symbol, decimals, minter) {
    const instantiateMsg = new MsgInstantiateContract(
        wallet.key.accAddress,
        wallet.key.accAddress,
        Number(code_id),
        {
            name,
            symbol,
            decimals,
            initial_balances: [],
            mint: {
                minter
            }
        }
    )
    const instantiateMsgTx = await wallet.createAndSignTx({
        msgs: [instantiateMsg]
    })

    const instantiateResult = await lcd.tx.broadcast(instantiateMsgTx)
    return remap_attributes(
        remap_events(instantiateResult.logs[0].events).instantiate_contract
    ).contract_address
}
//////////////////////////////////////////////////////////////////////
// exports
//////////////////////////////////////////////////////////////////////
module.exports = {
    upload_all_contracts,
    instantiate_prism_luna_hub_contract,
    instantiate_casset_token_contract
}
