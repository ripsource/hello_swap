mod helper;
use helper::*;
use scrypto::prelude::*;

// The following tests serve as examples and are not comprehensive by any means
#[derive(Debug, Clone, ScryptoSbor, NonFungibleData)]
pub struct OrderReceipt {
    pub order_qty: Decimal,
}
// The following tests serve as examples and are not comprehensive by any means

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_instantiate_price_one() {
        let res: ResourceAddress =
            ResourceBuilder::new_ruid_non_fungible::<OrderReceipt>(OwnerRole::None)
                .create_with_no_initial_supply()
                .address();
        instantiate_expect_success(res)
    }

    // #[test]
    // fn test_instantiate_price_zero() {
    //     instantiate_expect_failure(DEC_10, dec!(0));
    // }

    // #[test]
    // fn test_instantiate_price_negative() {
    //     instantiate_expect_failure(DEC_10, dec!(-1))
    // }
}
