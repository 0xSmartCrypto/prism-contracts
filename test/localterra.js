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
    logger.info('Grabbing account balances...')
    const balance = await lcd.bank.balance(mk.accAddress)
    logger.info(balance)
    logger.info('Attempting to upload contracts...')
    const contract_ids = await Helper.upload_all_contracts(lcd, wallet)
    logger.info(JSON.stringify(contract_ids))
}
run()
