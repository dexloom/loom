/*use std::fmt::{Display, Formatter};
use std::sync::Arc;

use alloy_primitives::{Address, Bytes, U256};
use eyre::{eyre, Report, Result};
use log::{debug, error, trace, warn};

use ethers::abi::{Abi, AbiEncode};
use ethers::prelude::{abigen, BlockId, BlockNumber, NameOrAddress};
use ethers::providers::Middleware;

use crate::Convert;
use crate::erc20::erc20::{ApproveCall, ERC20Calls, TotalSupplyCall};

pub struct CurveProtocol {}


abigen!(ICurveFactory, r#"[
            pool_list(uint256) external view returns (address)
            pool_count() external view returns (uint256)
        ]"#);

abigen!(ICurveAddressProvider, r#"[
            get_address(uint256) external view returns (address)
        ]"#);


abigen!(ICurveCommon, r#"[
            coins(uint256) external view returns (address)
            balances(uint256) external view returns (uint256)
            get_balances() external view returns (bytes)
        ]"#);

abigen!(ICurveCommonI128, r#"[
            coins(int128) external view returns (address)
            balances(int128) external view returns (uint256)
        ]"#);

abigen!(ICurveI128_2, r#"[
            get_dy(int128,int128,uint256) external view returns (uint256)
            calc_withdraw_one_coin(uint256,int128) external view returns (uint256)
            calc_token_amount(uint256[2],bool) external view returns (uint256)
            exchange(int128,int128,uint256,uint256) external
            remove_liquidity_one_coin(uint256,int128,uint256) external
            add_liquidity(uint256[2],uint256) external
        ]"#);


abigen!(ICurveI128_2_To_Meta, r#"[
            get_dy(int128,int128,uint256) external view returns (uint256)
            get_dy_underlying(int128,int128,uint256) external view returns (uint256)
            calc_withdraw_one_coin(uint256,int128) external view returns (uint256)
            calc_token_amount(uint256[2],bool) external view returns (uint256)
            exchange(int128,int128,uint256,uint256,address) external
            exchange_underlying(int128,int128,uint256,uint256,address) external
            remove_liquidity_one_coin(uint256,int128,uint256) external
            add_liquidity(uint256[2],uint256) external
        ]"#);

abigen!(ICurveI128_2_To, r#"[
            get_dy(int128,int128,uint256) external view returns (uint256)
            get_dx(int128,int128,uint256) external view returns (uint256)
            calc_withdraw_one_coin(uint256,int128) external view returns (uint256)
            calc_token_amount(uint256[2],bool) external view returns (uint256)
            exchange(int128,int128,uint256,uint256,address) external
            remove_liquidity_one_coin(uint256,int128,uint256) external
            add_liquidity(uint256[2],uint256) external
        ]"#);


abigen!(ICurveI128_3, r#"[
            get_dy(int128,int128,uint256) external view returns (uint256)
            calc_withdraw_one_coin(uint256,int128) external view returns (uint256)
            calc_token_amount(uint256[3],bool) external view returns (uint256)
            exchange(int128,int128,uint256,uint256) external
            remove_liquidity_one_coin(uint256,int128,uint256) external
            add_liquidity(uint256[3],uint256) external
        ]"#);

abigen!(ICurveI128_4, r#"[
            get_dy(int128,int128,uint256) external view returns (uint256)
            calc_token_amount(uint256[4],bool) external view returns (uint256)
            exchange(int128,int128,uint256,uint256) external
            add_liquidity(uint256[4],uint256) external
        ]"#);

abigen!(ICurveU256_2_To, r#"[
            get_dy(uint256,uint256,uint256) external view returns (uint256)
            calc_withdraw_one_coin(uint256,int128) external view returns (uint256)
            calc_token_amount(uint256[2],bool) external view returns (uint256)
            exchange(uint256,uint256,uint256,uint256,address) external
            remove_liquidity_one_coin(uint256,uint128,uint256) external
            add_liquidity(uint256[2],uint256) external
        ]"#);

abigen!(ICurveU256_2, r#"[
            get_dy(uint256,uint256,uint256) external view returns (uint256)
            calc_withdraw_one_coin(uint256,uint256) external view returns (uint256)
            calc_token_amount(uint256[2]) external view returns (uint256)
            exchange(uint256,uint256,uint256,uint256) external
            remove_liquidity_one_coin(uint256,uint256,uint256) external
            add_liquidity(uint256[2],uint256) external
        ]"#);


abigen!(ICurveU256_2_Eth_To, r#"[
            get_dy(uint256,uint256,uint256) external view returns (uint256)
            get_dx(uint256,uint256,uint256) external view returns (uint256)
            calc_withdraw_one_coin(uint256,int128) external view returns (uint256)
            calc_token_amount(uint256[2],bool) external view returns (uint256)
            exchange(uint256,uint256,uint256,uint256,bool,address) external
            remove_liquidity_one_coin(uint256,uint128,uint256) external
            add_liquidity(uint256[2],uint256) external
        ]"#);
abigen!(ICurveU256_3_Eth, r#"[
            get_dy(uint256,uint256,uint256) external view returns (uint256)
            calc_withdraw_one_coin(uint256,int128) external view returns (uint256)
            calc_token_amount(uint256[3],bool) external view returns (uint256)
            exchange(uint256,uint256,uint256,uint256,bool) external
            remove_liquidity_one_coin(uint256,uint256,uint256) external
            add_liquidity(uint256[3],uint256) external
        ]"#);
abigen!(ICurveU256_3_Eth_To, r#"[
            get_dy(uint256,uint256,uint256) external view returns (uint256)
            get_dx(uint256,uint256,uint256) external view returns (uint256)
            calc_token_amount(uint256[3],bool) external view returns (uint256)
            calc_withdraw_one_coin(uint256,int128) external  view returns (uint256)
            exchange(uint256,uint256,uint256,uint256,bool,address) external
            remove_liquidity_one_coin(uint256,uint256,uint256) external
            add_liquidity(uint256[3],uint256) external
        ]"#);
abigen!(ICurveU256_3_Eth_To2, r#"[
            get_dy(uint256,uint256,uint256) external view returns (uint256)
            get_dx(uint256,uint256,uint256) external view returns (uint256)
            calc_token_amount(uint256[3], bool) external view returns (uint256)
            calc_withdraw_one_coin(uint256,uint256) external  view returns (uint256)
            exchange(uint256,uint256,uint256,uint256,bool,address) external
            remove_liquidity_one_coin(uint256,uint256,uint256) external
            add_liquidity(uint256[3],uint256) external
        ]"#);


#[derive(Clone)]
pub enum CurveContract<M>
    where M: Middleware + 'static
{
    I128_2(ICurveI128_2<M>),
    I128_2_To(ICurveI128_2_To<M>),
    I128_2_To_Meta(ICurveI128_2_To_Meta<M>),
    I128_3(ICurveI128_3<M>),
    I128_4(ICurveI128_4<M>),
    U256_2(ICurveU256_2<M>),
    U256_2_To(ICurveU256_2_To<M>),
    U256_2_Eth_To(ICurveU256_2_Eth_To<M>),
    U256_3_Eth(ICurveU256_3_Eth<M>),
    U256_3_Eth_To(ICurveU256_3_Eth_To<M>),
    U256_3_Eth_To2(ICurveU256_3_Eth_To2<M>),
}

impl<M: Middleware + 'static> Display for CurveContract<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let contract_type = match self {
            CurveContract::I128_2(_) => "I128_2",
            CurveContract::I128_2_To(_) => "I128_2_To",
            CurveContract::I128_2_To_Meta(_) => "I128_2_To_Meta",
            CurveContract::I128_3(_) => "I128_3",
            CurveContract::I128_4(_) => "I128_4",
            CurveContract::U256_2(_) => "U256_2",
            CurveContract::U256_2_To(_) => "U256_2_To",
            CurveContract::U256_2_Eth_To(_) => "U256_2_Eth_To",
            CurveContract::U256_3_Eth(_) => "U256_3_Eth",
            CurveContract::U256_3_Eth_To(_) => "U256_3_Eth_To",
            CurveContract::U256_3_Eth_To2(_) => "U256_3_Eth_To2",
            _ => "CurveUnknown"
        };
        write!(f, "{}", contract_type)
    }
}

pub struct CurveCommonContract {}

impl CurveCommonContract {
    pub async fn lp_token<M: Middleware + 'static>(client: Arc<M>, address: Address) -> Result<Address> {
        if address == "0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7".parse::<Address>().unwrap() {
            return Ok("0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490".parse::<Address>().unwrap());
        }
        Err(eyre!("NO_LP_TOKEN"))
    }


    pub async fn coin<M: Middleware + 'static>(client: Arc<M>, address: Address, coin_id: u32) -> Result<Address> {
        let common_contract = ICurveCommon::new::<ethers::types::Address>(address.convert(), client.clone());
        match common_contract.coins(coin_id.into()).await {
            Ok(addr) => Ok(addr.convert()),
            Err(e) => {
                let common_contract = ICurveCommonI128::new::<ethers::types::Address>(address.convert(), client);
                match common_contract.coins(coin_id.into()).await {
                    Ok(addr) => Ok(addr.convert()),
                    Err(e) => {
                        trace!("coin call error {}", e);
                        Err(eyre!("CANNOT_GET_COIN_ADDRESS"))
                    }
                }
            }
        }
    }

    pub async fn balance<M: Middleware + 'static>(client: Arc<M>, address: Address, coin_id: u32) -> Result<U256> {
        let common_contract = ICurveCommon::new::<ethers::types::Address>(address.convert(), client.clone());
        match common_contract.balances(coin_id.into()).await {
            Ok(balance) => Ok(balance.convert()),
            Err(e) => {
                let common_contract = ICurveCommonI128::new::<ethers::types::Address>(address.convert(), client);
                match common_contract.balances(coin_id.into()).await {
                    Ok(balance) => Ok(balance.convert()),
                    Err(e) => {
                        trace!("coin call error {}", e);
                        Err(eyre!("CANNOT_GET_COIN_ADDRESS"))
                    }
                }
            }
        }
    }
    pub async fn coins<M: Middleware + 'static>(client: Arc<M>, address: Address) -> Result<Vec<Address>> {
        let mut ret: Vec<Address> = Vec::new();
        for i in 0..4 {
            match Self::coin(client.clone(), address, i).await {
                Ok(coint_address) => ret.push(coint_address),
                Err(_) => break,
            }
        }
        if ret.len() == 0 {
            trace!("coin fetch coins");
            Err(eyre!("CANNOT_GET_COIN_ADDRESS"))
        } else {
            trace!("coins @{} {:?}", address, ret );
            Ok(ret)
        }
    }


    pub async fn balances<M: Middleware + 'static>(client: Arc<M>, address: Address) -> Result<Vec<U256>> {
        let mut ret: Vec<U256> = Vec::new();

        let common_contract = ICurveCommon::new::<ethers::types::Address>(address.convert(), client.clone());
        match common_contract.get_balances().await {
            Ok(bytes) => {
                if bytes.len() < 64 {
                    return Err(eyre!("CANNOT_FETCH_BALANCES"));
                }
                let balances_count = U256::from_be_slice(&bytes.to_vec()[0..32]);
                for i in 0usize..balances_count.to() {
                    let balance = U256::from_be_slice(&bytes.to_vec()[32 + i * 32..64 + i * 32]);
                    ret.push(balance)
                }
                debug!("Curve Balances {:?} {:?}", address, ret);
                Ok(ret)
            }
            Err(e) => {
                for i in 0..4 {
                    match Self::balance(client.clone(), address, i).await {
                        Ok(balance) => ret.push(balance),
                        Err(e) => {
                            trace!("Error fetching coin balance {} : {}", i, e);
                            break;
                        }
                    }
                }
                if ret.len() == 0 {
                    trace!("coin call error {}", e);
                    Err(eyre!("CANNOT_GET_COIN_BALANCE"))
                } else {
                    Ok(ret)
                }
            }
        }
    }
}

impl<M: Middleware + 'static> CurveContract<M> {
    pub fn get_address(&self) -> Address {
        match self {
            CurveContract::I128_2(interface) => {
                interface.address()
            }
            CurveContract::I128_2_To_Meta(interface) => {
                interface.address()
            }
            CurveContract::I128_2_To(interface) => {
                interface.address()
            }
            CurveContract::I128_3(interface) => {
                interface.address()
            }
            CurveContract::I128_4(interface) => {
                interface.address()
            }
            CurveContract::U256_2(interface) => {
                interface.address()
            }
            CurveContract::U256_2_To(interface) => {
                interface.address()
            }
            CurveContract::U256_2_Eth_To(interface) => {
                interface.address()
            }
            CurveContract::U256_3_Eth(interface) => {
                interface.address()
            }
            CurveContract::U256_3_Eth_To(interface) => {
                interface.address()
            }
            CurveContract::U256_3_Eth_To2(interface) => {
                interface.address()
            }
        }.convert()
    }

    /*
        pub async fn coins(&self)->Result<Vec<Address>> {
            let mut coins : Vec<Address> = Vec::new();
            match &self{
                CurveContract::I128_2(interface) => {
                    for i in 0..2 {
                        match interface.coins(i.into()).call().await {
                            Ok(addr) => {
                                coins.push(addr);
                            }
                            _=>{
                                break
                            }
                        }
                    }
                }
                CurveContract::I128_2_To_Meta(interface) => {
                    for i in 0..2 {
                        match interface.coins(i.into()).call().await {
                            Ok(addr) => {
                                coins.push(addr);
                            }
                            _=>{
                                break
                            }
                        }
                    }
                }
                CurveContract::I128_2_To(interface) => {
                    for i in 0..2 {
                        match interface.coins(i.into()).call().await {
                            Ok(addr) => {
                                coins.push(addr);
                            }
                            _=>{
                                break
                            }
                        }
                    }
                }
                CurveContract::I128_3(interface) => {
                    for i in 0..3 {
                        match interface.coins(i.into()).call().await {
                            Ok(addr) => {
                                coins.push(addr);
                            }
                            _=>{
                                break
                            }
                        }
                    }
                }
                CurveContract::U256_2_To(interface) => {
                    for i in 0..2 {
                        match interface.coins(i.into()).call().await {
                            Ok(addr) => {
                                coins.push(addr);
                            }
                            _=>{
                                break
                            }
                        }
                    }
                }
                CurveContract::U256_2_Eth_To(interface) => {
                    for i in 0..2 {
                        match interface.coins(i.into()).call().await {
                            Ok(addr) => {
                                coins.push(addr);
                            }
                            _=>{
                                break
                            }
                        }
                    }
                }
                CurveContract::U256_3_Eth(interface) => {
                    for i in 0..3 {
                        match interface.coins(i.into()).call().await {
                            Ok(addr) => {
                                coins.push(addr);
                            }
                            _=>{
                                break
                            }
                        }
                    }
                }
                CurveContract::U256_3_Eth_To(interface) => {
                    for i in 0..3 {
                        match interface.coins(i.into()).call().await {
                            Ok(addr) => {
                                coins.push(addr);
                            }
                            _=>{
                                break
                            }
                        }
                    }
                }
                _=>{
                    return Err(eyre!("NOT_IMPLEMENTED"));
                }
            }

            Ok(coins)
        }
     */

    /*
        pub async fn balances(&self)->Result<Vec<U256>> {
            match self {
                CurveContract::I128_3(interface)=>{
                    let mut balances : Vec<U256> = Vec::new();
                    for i in 0..3 {
                        match interface.balances(i.into()).await {
                            Ok(x)=>balances.push(x),
                            Err(e)=>{
                                return Err(eyre!("CURVE_BALANCE_CALL_FAILED"))
                            }
                        }
                    }
                    Ok(balances)

                }
                CurveContract::I128_2(interface)=>{
                    let mut balances : Vec<U256> = Vec::new();
                    for i in 0..2 {
                        match interface.balances(i.into()).await {
                            Ok(x)=>balances.push(x),
                            Err(e)=>{
                                return Err(eyre!("CURVE_BALANCE_CALL_FAILED"))
                            }
                        }
                    }
                    Ok(balances)
                }
                CurveContract::I128_2_To_Meta(interface)=>{
                    match interface.get_balances().await {
                        Ok(x)=>Ok(x.to_vec()),
                        Err(e)=>return Err(eyre!("CURVE_BALANCE_CALL_FAILED"))
                    }
                }
                CurveContract::I128_2_To(interface)=>{
                    match interface.get_balances().await {
                        Ok(x)=>Ok(x.to_vec()),
                        Err(e)=>return Err(eyre!("CURVE_BALANCE_CALL_FAILED"))
                    }
                }
                CurveContract::U256_2_To(interface)=>{
                    let mut balances : Vec<U256> = Vec::new();
                    for i in 0..2 {
                        match interface.balances(i.into()).await {
                            Ok(x)=>balances.push(x),
                            Err(e)=>{
                                return Err(eyre!("CURVE_BALANCE_CALL_FAILED"))
                            }
                        }
                    }
                    Ok(balances)
                }
                CurveContract::U256_2_Eth_To(interface)=>{
                    let mut balances : Vec<U256> = Vec::new();
                    for i in 0..2 {
                        match interface.balances(i.into()).await {
                            Ok(x)=>balances.push(x),
                            Err(e)=>{
                                return Err(eyre!("CURVE_BALANCE_CALL_FAILED"))
                            }
                        }
                    }
                    Ok(balances)
                }
                CurveContract::U256_3_Eth(interface)=>{
                    let mut balances : Vec<U256> = Vec::new();
                    for i in 0..3 {
                        match interface.balances(i.into()).await {
                            Ok(x)=>balances.push(x),
                            Err(e)=>{
                                return Err(eyre!("CURVE_BALANCE_CALL_FAILED"))
                            }
                        }
                    }
                    Ok(balances)
                }
                CurveContract::U256_3_Eth_To(interface)=>{
                    let mut balances : Vec<U256> = Vec::new();
                    for i in 0..3 {
                        match interface.balances(i.into()).await {
                            Ok(x)=>balances.push(x),
                            Err(e)=>{
                                return Err(eyre!("CURVE_BALANCE_CALL_FAILED"))
                            }
                        }
                    }
                    Ok(balances)
                }
            }

        }

     */


    pub fn can_exchange_to(&self) -> bool {
        match self {
            CurveContract::I128_3(_) | CurveContract::U256_3_Eth(_) => {
                false
            }
            _ => true
        }
    }

    pub fn can_calculate_in_amount(&self) -> bool {
        match self {
            CurveContract::I128_2_To(_) | CurveContract::U256_2_Eth_To(_) | CurveContract::U256_3_Eth_To(_) | CurveContract::I128_2_To_Meta(_) => {
                true
            }
            _ => false
        }
    }


    pub async fn get_dy(&self, i: u32, j: u32, amount: U256) -> Result<U256> {
        match self {
            CurveContract::I128_2(interface) => {
                match interface.get_dy(i.into(), j.into(), amount.convert()).call().await {
                    Ok(x) => Ok(x.convert()),
                    _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR"))
                }
            }
            CurveContract::I128_2_To(interface) => {
                match interface.get_dy(i.into(), j.into(), amount.convert()).call().await {
                    Ok(x) => Ok(x.convert()),
                    _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR"))
                }
            }
            CurveContract::I128_2_To_Meta(interface) => {
                match interface.get_dy(i.into(), j.into(), amount.convert()).call().await {
                    Ok(x) => Ok(x.convert()),
                    _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR"))
                }
            }
            CurveContract::I128_3(interface) => {
                match interface.get_dy(i.into(), j.into(), amount.convert()).call().await {
                    Ok(x) => Ok(x.convert()),
                    _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR"))
                }
            }
            CurveContract::I128_4(interface) => {
                match interface.get_dy(i.into(), j.into(), amount.convert()).call().await {
                    Ok(x) => Ok(x.convert()),
                    _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR"))
                }
            }
            CurveContract::U256_2(interface) => {
                match interface.get_dy(i.into(), j.into(), amount.convert()).call().await {
                    Ok(x) => Ok(x.convert()),
                    _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR"))
                }
            }
            CurveContract::U256_2_To(interface) => {
                match interface.get_dy(i.into(), j.into(), amount.convert()).call().await {
                    Ok(x) => Ok(x.convert()),
                    _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR"))
                }
            }
            CurveContract::U256_2_Eth_To(interface) => {
                match interface.get_dy(i.into(), j.into(), amount.convert()).call().await {
                    Ok(x) => Ok(x.convert()),
                    _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR"))
                }
            }
            CurveContract::U256_3_Eth_To(interface) => {
                match interface.get_dy(i.into(), j.into(), amount.convert()).call().await {
                    Ok(x) => Ok(x.convert()),
                    _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR"))
                }
            }
            CurveContract::U256_3_Eth_To2(interface) => {
                match interface.get_dy(i.into(), j.into(), amount.convert()).call().await {
                    Ok(x) => Ok(x.convert()),
                    _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR"))
                }
            }
            CurveContract::U256_3_Eth(interface) => {
                match interface.get_dy(i.into(), j.into(), amount.convert()).call().await {
                    Ok(x) => Ok(x.convert()),
                    _ => Err(eyre!("CURVE_GET_DY_CALL_ERROR"))
                }
            }

            _ => {
                Err(eyre!("NOT_IMPLEMENTED"))
            }
        }
    }

    pub fn get_dx_call_data(&self, i: u32, j: u32, amount: U256) -> Result<Bytes> {
        let amount: ::ethers::types::U256 = amount.convert();
        let ret: Result<::ethers::types::Bytes, Report> = match self {
            /*CurveContract::I128_2_To_Meta(interface) => {
                match interface.get_dx(i.into(), j.into(), amount).calldata() {
                    Some(x)=>Ok(x),
                    _=>Err(eyre!("CURVE_DX_CALL_DATA_ERROR"))
                }
            }
             */
            CurveContract::I128_2_To(interface) => {
                match interface.get_dx(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DX_CALL_DATA_ERROR"))
                }
            }
            CurveContract::U256_3_Eth_To(interface) => {
                match interface.get_dx(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DX_CALL_DATA_ERROR"))
                }
            }
            CurveContract::U256_3_Eth_To2(interface) => {
                match interface.get_dx(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DX_CALL_DATA_ERROR"))
                }
            }
            _ => Err(eyre!("CURVE_CANNOT_CALC_DX"))
        };
        ret.map(|x| x.convert())
    }


    pub fn get_dy_underlying_call_data(&self, i: u32, j: u32, amount: U256) -> Result<Bytes> {
        match self {
            CurveContract::I128_2_To_Meta(interface) => {
                match interface.get_dy_underlying(i.into(), j.into(), amount.convert()).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DY_UNDERLYING_CALL_DATA_ERROR"))
                }
            }
            _ => {
                Err(eyre!("GET_DY_UNDERLYING_NOT_SUPPORTED"))
            }
        }.map(|x| x.convert())
    }

    pub fn get_dy_call_data(&self, i: u32, j: u32, amount: U256) -> Result<Bytes> {
        let amount: ::ethers::types::U256 = amount.convert();
        match self {
            CurveContract::I128_2(interface) => {
                match interface.get_dy(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DY_CALL_DATA_ERROR"))
                }
            }
            CurveContract::I128_2_To_Meta(interface) => {
                match interface.get_dy(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DY_CALL_DATA_ERROR"))
                }
            }
            CurveContract::I128_2_To(interface) => {
                match interface.get_dy(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DY_CALL_DATA_ERROR"))
                }
            }
            CurveContract::I128_3(interface) => {
                match interface.get_dy(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DY_CALL_DATA_ERROR"))
                }
            }
            CurveContract::I128_4(interface) => {
                match interface.get_dy(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DY_CALL_DATA_ERROR"))
                }
            }
            CurveContract::U256_2(interface) => {
                match interface.get_dy(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DY_CALL_DATA_ERROR"))
                }
            }
            CurveContract::U256_2_To(interface) => {
                match interface.get_dy(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DY_CALL_DATA_ERROR"))
                }
            }
            CurveContract::U256_2_Eth_To(interface) => {
                match interface.get_dy(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DY_CALL_DATA_ERROR"))
                }
            }
            CurveContract::U256_3_Eth(interface) => {
                match interface.get_dy(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DY_CALL_DATA_ERROR"))
                }
            }
            CurveContract::U256_3_Eth_To(interface) => {
                match interface.get_dy(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DY_CALL_DATA_ERROR"))
                }
            }
            CurveContract::U256_3_Eth_To2(interface) => {
                match interface.get_dy(i.into(), j.into(), amount).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_DY_CALL_DATA_ERROR"))
                }
            }
        }.map(|x| x.convert())
    }

    pub fn calc_token_amount_call_data(&self, i: u32, amount: U256) -> Result<Bytes> {
        let amount: ::ethers::types::U256 = amount.convert();
        match self {
            CurveContract::I128_2(interface) => {
                let mut amounts: [::ethers::types::U256; 2] = Default::default();
                amounts[i as usize] = amount;
                match interface.calc_token_amount(amounts, true).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_TOKEN_AMOUNT_CALL_DATA_ERROR"))
                }
            }
            CurveContract::I128_3(interface) => {
                let mut amounts: [::ethers::types::U256; 3] = Default::default();
                amounts[i as usize] = amount;
                match interface.calc_token_amount(amounts, true).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_TOKEN_AMOUNT_CALL_DATA_ERROR"))
                }
            }
            CurveContract::I128_4(interface) => {
                let mut amounts: [::ethers::types::U256; 4] = Default::default();
                amounts[i as usize] = amount;
                match interface.calc_token_amount(amounts, true).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_TOKEN_AMOUNT_CALL_DATA_ERROR"))
                }
            }
            _ => {
                Err(eyre!("CURVE_TOKEN_AMOUNT_CALL_DATA_NOT_SUPPORTED"))
            }
        }.map(|x| x.convert())
    }

    pub fn calc_withdraw_one_coin_call_data(&self, i: u32, amount: U256) -> Result<Bytes> {
        match self {
            CurveContract::I128_3(interface) => {
                match interface.calc_withdraw_one_coin(amount.convert(), i.into()).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_WITHDRAW_ONE_COIN_CALL_DATA_ERROR"))
                }
            }
            _ => {
                Err(eyre!("CURVE_WITHDRAW_ONE_COIN_NOT_SUPPORTED"))
            }
        }.map(|x| x.convert())
    }

    pub fn get_exchange_underlying_call_data(&self, i: u32, j: u32, amount: U256, min_dy: U256, to: Address) -> Result<Bytes> {
        match self {
            CurveContract::I128_2_To_Meta(interface) => {
                match interface.exchange_underlying(i.into(), j.into(), amount.convert(), min_dy.convert(), to.convert()).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_CALL_ERROR"))
                }
            }
            _ => {
                Err(eyre!("GET_EXCHANGE_UNDERLYING_CALL_DATA_NOT_SUPPORTED"))
            }
        }.map(|x| x.convert())
    }


    pub fn get_exchange_call_data(&self, i: u32, j: u32, amount: U256, min_dy: U256, to: Address) -> Result<Bytes> {
        let amount: ::ethers::types::U256 = amount.convert();
        let min_dy: ::ethers::types::U256 = min_dy.convert();
        let to: ::ethers::types::Address = to.convert();
        match self {
            CurveContract::I128_2(interface) => {
                match interface.exchange(i.into(), j.into(), amount, min_dy).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_CALL_ERROR"))
                }
            }
            CurveContract::I128_2_To(interface) => {
                match interface.exchange(i.into(), j.into(), amount, min_dy, to).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_CALL_ERROR"))
                }
            }
            CurveContract::I128_2_To_Meta(interface) => {
                match interface.exchange(i.into(), j.into(), amount, min_dy, to).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_CALL_ERROR"))
                }
            }
            CurveContract::I128_3(interface) => {
                match interface.exchange(i.into(), j.into(), amount, min_dy).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_CALL_ERROR"))
                }
            }
            CurveContract::I128_4(interface) => {
                match interface.exchange(i.into(), j.into(), amount, min_dy).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_CALL_ERROR"))
                }
            }
            CurveContract::U256_2(interface) => {
                match interface.exchange(i.into(), j.into(), amount, min_dy).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_CALL_ERROR"))
                }
            }
            CurveContract::U256_2_To(interface) => {
                match interface.exchange(i.into(), j.into(), amount, min_dy, to).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_CALL_ERROR"))
                }
            }
            CurveContract::U256_2_Eth_To(interface) => {
                match interface.exchange(i.into(), j.into(), amount, min_dy, false, to).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_CALL_ERROR"))
                }
            }
            CurveContract::U256_3_Eth(interface) => {
                match interface.exchange(i.into(), j.into(), amount, min_dy, false).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_CALL_ERROR"))
                }
            }
            CurveContract::U256_3_Eth_To(interface) => {
                match interface.exchange(i.into(), j.into(), amount, min_dy, false, to).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_CALL_ERROR"))
                }
            }
            CurveContract::U256_3_Eth_To2(interface) => {
                match interface.exchange(i.into(), j.into(), amount, min_dy, false, to).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("CURVE_CALL_ERROR"))
                }
            }
        }.map(|x| x.convert())
    }

    pub fn get_add_liquidity_call_data(&self, i: u32, amount: U256, to: Address) -> Result<Bytes> {
        match self {
            CurveContract::I128_3(interface) => {
                let mut amounts: [::ethers::types::U256; 3] = Default::default();
                amounts[i as usize] = amount.convert();
                match interface.add_liquidity(amounts, U256::ZERO.convert()).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("ADD_LIQUIDITY_CALL_ERROR"))
                }
            }
            _ => { Err(eyre!("ADD_LIQUIDITY_NOT_SUPPORTED")) }
        }.map(|x| x.convert())
    }

    pub fn get_remove_liquidity_one_coin_call_data(&self, i: u32, amount: U256, to: Address) -> Result<Bytes> {
        match self {
            CurveContract::I128_3(interface) => {
                match interface.remove_liquidity_one_coin(amount.convert(), i.into(), U256::ZERO.convert()).calldata() {
                    Some(x) => Ok(x),
                    _ => Err(eyre!("REMOVE_LIQUIDITY_ONE_COIN_ERROR"))
                }
            }
            _ => { Err(eyre!("REMOVE_LIQUIDITY_ONE_COIN_NOT_SUPPORTED")) }
        }.map(|x| x.convert())
    }
}


impl CurveProtocol {
    pub fn get_underlying_tokens(meta_token_address: Address) -> Result<Vec<Address>> {
        if meta_token_address == "0x6c3F90f043a72FA612cbac8115EE7e52BDe6E490".parse::<Address>().unwrap() {
            Ok(
                vec![
                    "0x6B175474E89094C44Da98b954EedeAC495271d0F".parse().unwrap(),
                    "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap(),
                    "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap(),
                ]
            )
        } else if meta_token_address == "0x3175Df0976dFA876431C2E9eE6Bc45b65d3473CC".parse::<Address>().unwrap() {
            Ok(
                vec![
                    "0x853d955aCEf822Db058eb8505911ED77F175b99e".parse().unwrap(),
                    "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap(),
                ]
            )
        } else {
            Err(eyre!("META_POOL_NOT_FOUND"))
        }
    }


    pub fn new_i128_2<M: Middleware + 'static>(client: Arc<M>, address: Address) -> CurveContract<M> {
        let contract = ICurveI128_2::new::<::ethers::types::Address>(address.convert(), client);
        CurveContract::I128_2(contract)
    }

    pub fn new_i128_2_to<M: Middleware + 'static>(client: Arc<M>, address: Address) -> CurveContract<M> {
        let contract = ICurveI128_2_To::new::<::ethers::types::Address>(address.convert(), client);
        CurveContract::I128_2_To(contract)
    }
    pub fn new_i128_2_to_meta<M: Middleware + 'static>(client: Arc<M>, address: Address) -> CurveContract<M> {
        let contract = ICurveI128_2_To_Meta::new::<::ethers::types::Address>(address.convert(), client);
        CurveContract::I128_2_To_Meta(contract)
    }
    pub fn new_i128_3<M: Middleware + 'static>(client: Arc<M>, address: Address) -> CurveContract<M> {
        let contract = ICurveI128_3::new::<::ethers::types::Address>(address.convert(), client);
        CurveContract::I128_3(contract)
    }
    pub fn new_i128_4<M: Middleware + 'static>(client: Arc<M>, address: Address) -> CurveContract<M> {
        let contract = ICurveI128_4::new::<::ethers::types::Address>(address.convert(), client);
        CurveContract::I128_4(contract)
    }
    pub fn new_u256_2<M: Middleware + 'static>(client: Arc<M>, address: Address) -> CurveContract<M> {
        let contract = ICurveU256_2::new::<::ethers::types::Address>(address.convert(), client);
        CurveContract::U256_2(contract)
    }

    pub fn new_u256_2_to<M: Middleware + 'static>(client: Arc<M>, address: Address) -> CurveContract<M> {
        let contract = ICurveU256_2_To::new::<ethers::types::Address>(address.convert(), client);
        CurveContract::U256_2_To(contract)
    }

    pub fn new_u256_2_eth_to<M: Middleware + 'static>(client: Arc<M>, address: Address) -> CurveContract<M> {
        let contract = ICurveU256_2_Eth_To::new::<ethers::types::Address>(address.convert(), client);
        CurveContract::U256_2_Eth_To(contract)
    }

    pub fn new_u256_3_eth_to<M: Middleware + 'static>(client: Arc<M>, address: Address) -> CurveContract<M> {
        let contract = ICurveU256_3_Eth_To::new::<ethers::types::Address>(address.convert(), client);
        CurveContract::U256_3_Eth_To(contract)
    }
    pub fn new_u256_3_eth_to2<M: Middleware + 'static>(client: Arc<M>, address: Address) -> CurveContract<M> {
        let contract = ICurveU256_3_Eth_To2::new::<ethers::types::Address>(address.convert(), client);
        CurveContract::U256_3_Eth_To2(contract)
    }

    pub fn new_u256_3_eth<M: Middleware + 'static>(client: Arc<M>, address: Address) -> CurveContract<M> {
        let contract = ICurveU256_3_Eth::new::<ethers::types::Address>(address.convert(), client);
        CurveContract::U256_3_Eth(contract)
    }
    /*
    I128_3
    0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7 // DAI-USDT-USDC

    I128_2_to
    0x4DEcE678ceceb27446b35C672dC7d61F30bAD69E // crvUSD-USDC

    U256_2_Eth_to
    0x9409280DC1e6D33AB7A8C6EC03e5763FB61772B5  // LDO-ETH

    U256_3_Eth
    0xD51a44d3FaE010294C616388b506AcdA1bfAAE46 // WETH-WBTC-USDT


    U256_3_Eth_to
    0x7F86Bf177Dd4F3494b841a37e810A34dD56c829B // WETH-WBTC-USDC
    0xf5f5B97624542D72A9E06f04804Bf81baA15e2B4 // WETH-WBTC-USDT
     */

    fn match_abi(code: &Bytes, abi: &Abi) -> bool {
        //println!("Code len {}", code.len());
        for (fn_name, fxs) in abi.functions.iter() {
            for f in fxs.iter() {
                if !code.as_ref().windows(4).any(|sig| sig == &f.short_signature()) {
                    //println!("{} not found", fn_name);
                    return false;
                } else {
                    //println!("{} found", fn_name);
                }
            }
        }
        true
    }

    pub async fn get_factory_address<M: Middleware + 'static>(client: Arc<M>, id: u32) -> Result<Address> {
        let address_provider_address: Address = "0x0000000022D53366457F9d5E68Ec105046FC4383".parse().unwrap();
        let address_provider = ICurveAddressProvider::new::<ethers::types::Address>(address_provider_address.convert(), client);
        match address_provider.get_address(id.into()).await {
            Ok(x) => Ok(x.convert()),
            Err(e) => {
                error!("Error getting factory address");
                Err(eyre!("GET__FACTORY_ADDRESS_ERROR"))
            }
        }
    }

    pub async fn get_pool_address<M: Middleware + 'static>(client: Arc<M>, factory_address: Address, pool_id: u32) -> Result<Address> {
        let factory = ICurveFactory::new::<ethers::types::Address>(factory_address.convert(), client);
        match factory.pool_list(pool_id.into()).await {
            Ok(x) => Ok(x.convert()),
            Err(e) => {
                error!("Error getting factory address");
                Err(eyre!("GET_POOL_ADDRESS_ERROR"))
            }
        }
    }

    pub async fn get_pool_count<M: Middleware + 'static>(client: Arc<M>, factory_address: Address) -> Result<u32> {
        let factory = ICurveFactory::new::<ethers::types::Address>(factory_address.convert(), client);
        match factory.pool_count().await {
            Ok(x) => Ok(x.as_u32()),
            Err(e) => {
                error!("Error getting pool count");
                Err(eyre!("GET_POOL_COUNT_ERROR"))
            }
        }
    }


    pub async fn get_contract_from_code<M: Middleware>(client: Arc<M>, address: Address) -> Result<CurveContract<M>> {
        //let sig = ICurveU256_3_EthCalls::Balances(  <ICurveU256_3_Eth<M>>::BalancesCall );
        //let sig = ICurveU256_3_EthCalls::Balances(  BalancesCall{} );

        let mut code = client.get_code::<NameOrAddress>(address.convert(), Some(BlockId::from(BlockNumber::Latest))).await?;

        if code.len() < 100 {
            for i in 20..code.len() - 1 {
                if code[i] == 0x5A && code[i + 1] == 0xF4 {
                    let underlying_address = Address::from_slice(&code.to_vec()[i - 20..i]);
                    println!("Underlying address {}", underlying_address);
                    code = client.get_code::<NameOrAddress>(underlying_address.convert(), Some(BlockId::from(BlockNumber::Latest))).await?;
                    break;
                }
            }
        }

        let code: Bytes = code.convert();

        if code.len() < 100 {
            return Err(eyre!("CANNOT_FIND_UNDERLYING"));
        }

        if Self::match_abi(&code, &ICURVEI128_2_TO_META_ABI) {
            return Ok(Self::new_i128_2_to_meta(client, address));
        }
        if Self::match_abi(&code, &ICURVEI128_2_TO_ABI) {
            return Ok(Self::new_i128_2_to(client, address));
        }
        if Self::match_abi(&code, &ICURVEI128_2_ABI) {
            return Ok(Self::new_i128_2(client, address));
        }
        if Self::match_abi(&code, &ICURVEI128_3_ABI) {
            return Ok(Self::new_i128_3(client, address));
        }
        if Self::match_abi(&code, &ICURVEI128_4_ABI) {
            return Ok(Self::new_i128_4(client, address));
        }
        if Self::match_abi(&code, &ICURVEU256_2_TO_ABI) {
            return Ok(Self::new_u256_2_to(client, address));
        }
        if Self::match_abi(&code, &ICURVEU256_2_ABI) {
            return Ok(Self::new_u256_2_to(client, address));
        }
        if Self::match_abi(&code, &ICURVEU256_2_ETH_TO_ABI) {
            return Ok(Self::new_u256_2_eth_to(client, address));
        }
        if Self::match_abi(&code, &ICURVEU256_3_ETH_ABI) {
            return Ok(Self::new_u256_3_eth(client, address));
        }
        if Self::match_abi(&code, &ICURVEU256_3_ETH_TO_ABI) {
            return Ok(Self::new_u256_3_eth_to(client, address));
        }
        if Self::match_abi(&code, &ICURVEU256_3_ETH_TO2_ABI) {
            return Ok(Self::new_u256_3_eth_to2(client, address));
        }

        Err(eyre!("ABI_NOT_FOUND"))
    }


    pub fn get_contracts_vec<M: Middleware>(client: Arc<M>) -> Vec<CurveContract<M>> {
        vec![
            Self::new_i128_3(client.clone(), "0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7".parse().unwrap()),
            Self::new_i128_2_to(client.clone(), "0x4DEcE678ceceb27446b35C672dC7d61F30bAD69E".parse().unwrap()),
            Self::new_u256_2_eth_to(client.clone(), "0x9409280DC1e6D33AB7A8C6EC03e5763FB61772B5".parse().unwrap()),
            Self::new_u256_3_eth(client.clone(), "0xD51a44d3FaE010294C616388b506AcdA1bfAAE46".parse().unwrap()),
            Self::new_u256_3_eth_to(client.clone(), "0x7F86Bf177Dd4F3494b841a37e810A34dD56c829B".parse().unwrap()),
            Self::new_u256_3_eth_to(client.clone(), "0xf5f5B97624542D72A9E06f04804Bf81baA15e2B4".parse().unwrap()),
            Self::new_i128_2_to_meta(client.clone(), "0xEd279fDD11cA84bEef15AF5D39BB4d4bEE23F0cA".parse().unwrap()),
            Self::new_i128_2(client.clone(), "0xDC24316b9AE028F1497c275EB9192a3Ea0f67022".parse().unwrap()),
            Self::new_i128_2_to(client.clone(), "0x828b154032950C8ff7CF8085D841723Db2696056".parse().unwrap()),
        ]
    }


    fn get_abi<M: Middleware + 'static>(client: Arc<M>, address: Address) -> Result<CurveContract<M>> {
        /*abigen!(ICurve2, r#"[
            coins(uint256)
        ]"#);
         */
        Err(eyre!("NE"))
    }
}

*/