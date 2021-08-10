//////////////////////////////////////////////////////////////////////
// prism-contracts - refracting defi
//////////////////////////////////////////////////////////////////////
const { LCDClient, MnemonicKey } = require('@terra-money/terra.js')
const Helper = require('./helpers/smart-contracts')
const moment = require('moment')
const winston = require('winston')
//////////////////////////////////////////////////////////////////////
// logger
//////////////////////////////////////////////////////////////////////
const logger = winston.createLogger({
    level: 'info',
    transports: [
        new winston.transports.Console({
            format: winston.format.combine(
                winston.format.colorize(),
                winston.format.printf(({
                    level, message
                }) => `[${moment().format()}][${level}]: ${message}`)
            )
        })
    ]
})
//////////////////////////////////////////////////////////////////////
// setup
//////////////////////////////////////////////////////////////////////
const lcd = new LCDClient({
    URL: 'http://localhost:1317',
    chainID: 'localterra',
})
const mk = new MnemonicKey({
    mnemonic:
      'notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius',
})
const wallet = lcd.wallet(mk)
//////////////////////////////////////////////////////////////////////
// run
//////////////////////////////////////////////////////////////////////
async function run() {
    logger.info(`Using account '${mk.accAddress}'...`)
    logger.info('Grabbing account balances...')
    const balance = await lcd.bank.balance(mk.accAddress)
    logger.info(balance)
    logger.info('Attempting to upload contracts...')
    const contract_ids = await Helper.upload_all_contracts(lcd, wallet)
    logger.info(JSON.stringify(contract_ids))
    logger.info('Instantiating prism_luna_hub contract...')
    const prism_luna_hub =
        await Helper.instantiate_prism_luna_hub_contract(lcd, wallet, contract_ids.prism_luna_hub)
    logger.info(`prism-luna-hub contract: ${prism_luna_hub}`)
    logger.info('Instantiating prism_cluna_cw20 contract...')
    const prism_cluna_asset =
        await Helper.instantiate_casset_token_contract(
            lcd,
            wallet,
            contract_ids.casset_token_contract,
            'cLuna',
            'CLUNA',
            6,
            prism_luna_hub
        )
    logger.info(`prism-cluna-cw20 contract: ${prism_cluna_asset}`)
    logger.info(`Updating prism_luna_hub contract parameters...`)
    const param_tx = await Helper.update_prism_luna_hub_contract(lcd, wallet, prism_luna_hub, prism_cluna_asset)
    logger.info(`TX: ${param_tx}`)
}
run()
