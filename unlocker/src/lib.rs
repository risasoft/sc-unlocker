#![no_std]

const PERCENTAGE_TOTAL: u32 = 10_000; // 100%
const MINIMUM_DEPOSIT: u64 = 1_000;

elrond_wasm::imports!();
#[elrond_wasm::derive::contract]
pub trait Unlocker {
    #[init]
    fn init(&self, from_token: TokenIdentifier, to_token: TokenIdentifier, fee_percent: u32) {
        self.try_set_fee_percentage(fee_percent);

        self.add_from_token(from_token);

        self.add_to_token(to_token)
    }

    #[payable("*")]
    #[endpoint(swap)]
    fn swap(
        &self,
        #[payment_token] token_id: TokenIdentifier,
        #[payment_amount] amount: BigUint,
    ) -> () {
        require!(amount >= PERCENTAGE_TOTAL, "amount too small");
        require!(!self.blockchain().get_caller().is_zero(), "invalid caller");
        require!(
            self.from_tokens().contains(&token_id),
            "token not supported"
        );

        let fee_percent = self.fee_percent().get();
        require!(&fee_percent > &0, "zero fee");

        let fee = self.calculate_percentage(&amount, &fee_percent);
        let amount_after_fee = &amount - &fee;

        require!(&amount_after_fee < &amount, "incorrect fee");
        require!(
            &amount_after_fee <= &self.get_liquidity_balance(),
            "no liquidity"
        );
        require!(&amount_after_fee > &0, "nothing to send");
        self.send().direct(
            &self.blockchain().get_caller(),
            &self.to_token().get(),
            0,
            &amount_after_fee,
            &[],
        );
    }

    #[payable("*")]
    #[endpoint(deposit)]
    fn deposit(
        &self,
        #[payment_token] token_id: TokenIdentifier,
        #[payment_amount] amount: BigUint,
    ) -> () {
        let caller = self.blockchain().get_caller();
        require!(!caller.is_zero(), "invalid caller");
        require!(self.to_token().get() == token_id, "token not supported");
        require!(amount > 0, "incorrect amount");
        require!(
            amount >= MINIMUM_DEPOSIT,
            "Deposit amount must be greater than or equal to minimum deposit"
        );

        let amount_with_fees = self.calculate_amount_with_fees(&amount);

        self.depositor_balance(&caller)
            .update(|balance| *balance += &amount_with_fees);
    }

    #[view(getLiquidityBalance)]
    fn get_liquidity_balance(&self) -> BigUint {
        self.blockchain().get_sc_balance(&self.to_token().get(), 0)
    }

    #[endpoint(harvest)]
    fn harvest(
        &self,
        token: TokenIdentifier,
        nonce: u64,
        amount: BigUint,
    ) -> () {
        let caller = self.blockchain().get_caller();
        require!(!caller.is_zero(), "invalid caller");

        let sc_balance = self.blockchain().get_sc_balance(&token, nonce);
        let dep_balance = self.depositor_balance(&caller).get();

        require!(self.from_tokens().contains(&token), "token not supported");
        require!(&amount > &0, "Invalid amount");
        require!(&sc_balance > &0, "Insufficient contract funds (0)");
        require!(&dep_balance > &0, "Insufficient depositor funds (0)");
        require!(&sc_balance >= &amount, "Insufficient sc funds");
        require!(&dep_balance >= &amount, "Insufficient depositor funds");

        self.send().direct(&caller, &token, nonce, &amount, &[]);

        self.depositor_balance(&caller)
            .update(|balance| *balance -= &amount);
    }

    // PRIVATE METHODS
    fn calculate_percentage(&self, total_amount: &BigUint, percentage: &BigUint) -> BigUint {
        total_amount * percentage / PERCENTAGE_TOTAL
    }
    fn calculate_amount_with_fees(&self, amount: &BigUint) -> BigUint {
        let fee_percent = self.fee_percent().get();
        let fee = self.calculate_percentage(&amount, &fee_percent);

        amount + &fee
    }

    // OWNER ENDPOINTS

    #[only_owner]
    #[endpoint(addFromToken)]
    fn add_from_token(&self, asset: TokenIdentifier) -> () {
        require!(asset.is_valid_esdt_identifier(), "Invalid ESDT");
        self.from_tokens().insert(asset);
    }

    #[only_owner]
    #[endpoint(setToToken)]
    fn add_to_token(&self, asset: TokenIdentifier) -> () {
        require!(asset.is_valid_esdt_identifier(), "Invalid ESDT");
        self.to_token().set(&asset);
    }

    #[only_owner]
    #[endpoint(setFee)]
    fn try_set_fee_percentage(&self, new_fee_percentage: u32) {
        require!(
            new_fee_percentage > 0 && new_fee_percentage < PERCENTAGE_TOTAL,
            "Invalid percentage value, should be between 0 and 10,000"
        );
        self.fee_percent().set(&BigUint::from(new_fee_percentage));
    }

    #[only_owner]
    #[endpoint(withdraw)]
    fn withdraw(&self, token: TokenIdentifier, nonce: u64) -> () {
        self.send().direct(
            &self.blockchain().get_owner_address(),
            &token,
            nonce,
            &self.blockchain().get_sc_balance(&token, nonce),
            &[],
        );
    }

    // STORAGE
    #[view(getFee)]
    #[storage_mapper("fee_percent")]
    fn fee_percent(&self) -> SingleValueMapper<BigUint>;

    #[storage_mapper("to_token")]
    fn to_token(&self) -> SingleValueMapper<TokenIdentifier>;

    #[view(getFromTokens)]
    #[storage_mapper("from_tokens")]
    fn from_tokens(&self) -> SetMapper<TokenIdentifier>;

    #[view(getBalance)]
    #[storage_mapper("depositor_balance")]
    fn depositor_balance(&self, address: &ManagedAddress) -> SingleValueMapper<BigUint>;
}
