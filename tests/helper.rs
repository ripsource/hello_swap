use lazy_static::lazy_static;
use radix_engine::{
    blueprints::package::PackageDefinition,
    system::system_modules::execution_trace::ResourceSpecifier::Amount,
};
use scrypto::prelude::*;
use scrypto_testenv::*;
use std::mem;
use transaction::builder::ManifestBuilder;

lazy_static! {
    static ref PACKAGE: (Vec<u8>, PackageDefinition) = compile_package(this_package!());
}

impl TestHelperExecution for HelloSwapTestHelper {
    fn env(&mut self) -> &mut TestEnvironment {
        &mut self.env
    }
}

pub struct HelloSwapTestHelper {
    env: TestEnvironment,
    component_address: Option<ComponentAddress>,
}

impl HelloSwapTestHelper {
    pub fn new() -> HelloSwapTestHelper {
        let env = TestEnvironment::new(vec![("hello_swap", &PACKAGE)]);

        HelloSwapTestHelper {
            env,
            component_address: None,
        }
    }

    pub fn instantiate(&mut self, x_address: ResourceAddress) -> &mut HelloSwapTestHelper {
        // with the next ManifestBuilder update this can be simplified to
        // let manifest_builder = mem::take(&mut self.environment.manifest_builder);
        let manifest_builder = mem::replace(&mut self.env.manifest_builder, ManifestBuilder::new());
        self.env.manifest_builder = manifest_builder.call_function(
            self.env.package_address("hello_swap"),
            "HelloSwap",
            "instantiate_bidbook_collection",
            manifest_args!(x_address),
        );
        // To support instruction labels we are tracking:
        // instruction_count = the total amount of new instructions added in this function
        // label_instruction_id = (local) instruction id which you want to assign to the label
        // after the ManifestBuilder supports labels upstream this can be simplified
        self.env.new_instruction("instantiate", 1, 2);
        self
    }

    // pub fn swap(
    //     &mut self,
    //     x_address: ResourceAddress,
    //     x_amount: Decimal,
    // ) -> &mut HelloSwapTestHelper {
    //     let manifest_builder = mem::replace(&mut self.env.manifest_builder, ManifestBuilder::new());
    //     self.env.manifest_builder = manifest_builder
    //         .withdraw_from_account(self.env.account, x_address, x_amount)
    //         .take_from_worktop(x_address, x_amount, self.name("x_bucket"))
    //         .with_name_lookup(|builder, lookup| {
    //             let x_bucket = lookup.bucket(self.name("x_bucket"));
    //             builder.call_method(self.pool_address.unwrap(), "swap", manifest_args!(x_bucket))
    //         });
    //     self.env.new_instruction("swap", 3, 2);
    //     self
    // }

    pub fn instantiate_default(&mut self, x_address: ResourceAddress, verbose: bool) -> Receipt {
        self.instantiate(x_address);
        let receipt = self.execute_expect_success(verbose);
        let component_address: ComponentAddress = receipt.outputs("instantiate")[0];
        self.component_address = Some(component_address);
        receipt
    }

    // pub fn swap_expect_failure(&mut self, x_amount: Decimal) {
    //     self.swap(self.x_address(), x_amount)
    //         .execute_expect_failure(true);
    // }

    // pub fn swap_expect_success(
    //     &mut self,
    //     x_amount: Decimal,
    //     y_amount_expected: Decimal,
    //     x_remainder_expected: Decimal,
    // ) {
    //     let receipt = self
    //         .swap(self.x_address(), x_amount)
    //         .execute_expect_success(true);
    //     let output_buckets = receipt.output_buckets("swap");

    //     assert_eq!(
    //         output_buckets,
    //         vec![vec![
    //             Amount(self.y_address(), y_amount_expected),
    //             Amount(self.x_address(), x_remainder_expected)
    //         ]],
    //     );
    // }

    // pub fn a_address(&self) -> ResourceAddress {
    //     self.env.a_address
    // }

    // pub fn b_address(&self) -> ResourceAddress {
    //     self.env.b_address
    // }

    // pub fn x_address(&self) -> ResourceAddress {
    //     self.env.x_address
    // }

    // pub fn y_address(&self) -> ResourceAddress {
    //     self.env.y_address
    // }

    // pub fn v_address(&self) -> ResourceAddress {
    //     self.env.v_address
    // }

    // pub fn u_address(&self) -> ResourceAddress {
    //     self.env.u_address
    // }

    // pub fn j_nft_address(&self) -> ResourceAddress {
    //     self.env.j_nft_address
    // }

    // pub fn k_nft_address(&self) -> ResourceAddress {
    //     self.env.k_nft_address
    // }
}

pub fn instantiate_expect_success(x_address: ResourceAddress) {
    let mut helper = HelloSwapTestHelper::new();
    helper.instantiate_default(x_address, true);
}

pub fn instantiate_expect_failure(x_address: ResourceAddress) {
    let mut helper = HelloSwapTestHelper::new();
    helper.instantiate(x_address).execute_expect_failure(true);
}

// pub fn swap_expect_success(
//     y_vault_amount: Decimal,
//     price: Decimal,
//     x_input: Decimal,
//     y_output_expected: Decimal,
//     x_remainder_expected: Decimal,
// ) {
//     let mut helper = HelloSwapTestHelper::new();
//     helper.instantiate_default(y_vault_amount, price, true);
//     helper.swap_expect_success(x_input, y_output_expected, x_remainder_expected);
// }

// pub fn swap_expect_failure(y_vault_amount: Decimal, price: Decimal, x_input: Decimal) {
//     let mut helper = HelloSwapTestHelper::new();
//     helper.instantiate_default(y_vault_amount, price, true);
//     helper.swap_expect_failure(x_input);
// }
