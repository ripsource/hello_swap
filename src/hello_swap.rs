use scrypto::prelude::*;
use scrypto_avltree::avl_tree::AvlTree;
use scrypto_avltree::avl_tree_health::{check_health, print_tree_nice};

#[derive(Clone, PartialEq, Debug, ScryptoSbor)]
pub enum OrderStatus {
    FILLED,
    PARTIAL,
    OPEN,
}

#[derive(Debug, PartialEq, Clone, ScryptoSbor)]
pub struct Order {
    pub order_id: NonFungibleGlobalId,
    pub order_qty: Decimal,
    pub order_price: Decimal,
    pub order_total: Decimal,
    pub order_time: i64,
    pub order_sequence: u64,
    pub order_status: OrderStatus,
}

#[derive(Debug, Clone, ScryptoSbor, NonFungibleData)]
pub struct OrderReceipt {
    pub order_qty: Decimal,
    pub order_price: Decimal,
    pub order_total: Decimal,
    pub order_time: i64,
    pub order_sequence: u64,
    pub order_status: OrderStatus,
}

#[derive(Debug, PartialEq, Clone, ScryptoSbor)]
pub struct OrderbookLine {
    pub level_price: Decimal,
    pub level_qty: u64,
    pub level_total: Decimal,
    pub level_orders: Vec<NonFungibleGlobalId>,
}

#[blueprint]
mod hello_swap {

    struct HelloSwap {
        price_levels: AvlTree<Decimal, ()>,
        orderbook_lines: KeyValueStore<Decimal, OrderbookLine>,
        orders: KeyValueStore<NonFungibleGlobalId, Order>,
        nft_vaults: KeyValueStore<NonFungibleGlobalId, Vault>,
        bid_vaults: KeyValueStore<NonFungibleGlobalId, Vault>,
        highest_bid: Decimal,
        lowest_bid: Decimal,
        latest_order: i64,
        sequence_number: u64,
        collection: ResourceAddress,
        receipt_generator: ResourceManager,
        receipt_resource_address: ResourceAddress,
    }

    impl HelloSwap {
        pub fn instantiate_collection_bidbook(collection: ResourceAddress) -> Global<HelloSwap> {
            let (bidbook_address_reservation, bidbook_component_address) =
                Runtime::allocate_component_address(HelloSwap::blueprint_id());

            let global_caller_badge_rule = rule!(require(global_caller(bidbook_component_address)));

            let receipt_generator =
                ResourceBuilder::new_ruid_non_fungible::<OrderReceipt>(OwnerRole::None)
                    .mint_roles(mint_roles! {
                        minter => global_caller_badge_rule.clone();
                        minter_updater => rule!(deny_all);
                    })
                    .non_fungible_data_update_roles(non_fungible_data_update_roles! {
                        non_fungible_data_updater => global_caller_badge_rule.clone();
                        non_fungible_data_updater_updater => rule!(deny_all);
                    })
                    .burn_roles(burn_roles! {
                        burner => global_caller_badge_rule.clone();
                        burner_updater => rule!(deny_all);
                    })
                    .create_with_no_initial_supply();

            let receipt_resource_address = receipt_generator.address();

            Self {
                price_levels: AvlTree::new(),
                orderbook_lines: KeyValueStore::new(),
                orders: KeyValueStore::new(),
                nft_vaults: KeyValueStore::new(),
                bid_vaults: KeyValueStore::new(),
                highest_bid: dec!(0),
                lowest_bid: dec!(0),
                latest_order: 0,
                sequence_number: 0,
                collection,
                receipt_generator,
                receipt_resource_address,
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::None)
            .with_address(bidbook_address_reservation)
            .globalize()
        }

        pub fn place_bid(&mut self, bid: FungibleBucket, order_price: Decimal) -> Bucket {
            // Checklist of all functions:
            // price_levels: AvlTree<Decimal, ()>,
            // orderbook_lines: KeyValueStore<Decimal, OrderbookLine>,
            // orders: KeyValueStore<NonFungibleGlobalId, Order>,
            // nft_vaults: KeyValueStore<NonFungibleGlobalId, Vault>,
            // bid_vaults: KeyValueStore<NonFungibleGlobalId, Vault>,
            // highest_bid: Decimal,

            assert!(
                order_price > dec!(0),
                "[Place Bid] : Order price must be greater than 0"
            );
            assert!(
                bid.amount() > dec!(0),
                "[Place Bid] : Bid amount must be greater than 0"
            );
            assert!(
                bid.resource_address() == XRD,
                "[Place Bid] : Bid must be in XRD"
            );

            // Get the XRD of the order - verify it's been submitted for the right price level and for a whole number of NFTs

            let order_qty = bid
                .amount()
                .checked_div(order_price)
                .unwrap()
                .checked_round(18, RoundingMode::ToNegativeInfinity)
                .unwrap();

            let s = order_qty.to_string();

            let decimal_place_check = if let Some(pos) = s.find('.') {
                s[pos + 1..].len() as u32
            } else {
                0
            };

            assert!(
                decimal_place_check == 0,
                "[Place Bid] : Bids can not be placed for a fraction of an NFT"
            );

            assert!(
                order_qty > dec!(0),
                "[Place Bid] : Order qty must be greater than 0"
            );

            let funds_added_to_orderbook = bid.amount();

            // Get the order time and sequence number if at the same time. If not, reset the sequence number to 0.

            let order_time = Clock::current_time(TimePrecision::Minute).seconds_since_unix_epoch;

            if order_time != self.latest_order {
                self.latest_order = order_time;
                self.sequence_number = 0;
            } else {
                self.sequence_number += 1;
            }

            let order_sequence_number = self.sequence_number;

            // Create a new order receipt NFT for the bid

            let receipt = self.receipt_generator.mint_ruid_non_fungible({
                OrderReceipt {
                    order_qty,
                    order_price,
                    order_total: bid.amount().clone(),
                    order_time,
                    order_sequence: order_sequence_number,
                    order_status: OrderStatus::OPEN,
                }
            });

            // Create a new vault for the XRD in the order

            let order_vault = Vault::with_bucket(bid.into());

            // Insert the the order_vault into the bid_vaults key value store using NFGID as the key

            let order_id = NonFungibleGlobalId::new(
                receipt.resource_address(),
                receipt.as_non_fungible().non_fungible_local_id(),
            );

            self.bid_vaults.insert(order_id.clone(), order_vault);

            // Insert a record of the order into the orders key value store using NFGID as the key

            let order_id_insert = order_id.clone();

            let order = Order {
                order_id: order_id_insert,
                order_qty,
                order_total: funds_added_to_orderbook.clone(),
                order_price,
                order_time,
                order_sequence: order_sequence_number,
                order_status: OrderStatus::OPEN,
            };

            self.orders.insert(order_id.clone(), order.clone());

            // check if price level already exists
            // if it doesn't create new price level and orderbook line
            // if it does, update the orderbook line at that price level

            let mut price_level_exists = false;

            if self.price_levels.get(&order_price).is_some() {
                price_level_exists = true;
            }

            if !price_level_exists {
                self.price_levels.insert(order_price, ());
            }

            let orderbook_line_exists = self.orderbook_lines.get(&order_price).is_some();

            if orderbook_line_exists {
                if let Some(mut orderbook_line) = self.orderbook_lines.get_mut(&order_price) {
                    orderbook_line.level_qty += 1;
                    orderbook_line.level_total += funds_added_to_orderbook;
                    orderbook_line.level_orders.push(order_id.clone());
                }
            } else {
                let new_orderbook_line = OrderbookLine {
                    level_price: order_price,
                    level_qty: 1,
                    level_total: funds_added_to_orderbook,
                    level_orders: vec![order_id.clone()],
                };

                self.orderbook_lines.insert(order_price, new_orderbook_line);
            }

            // Update the highest bid if the new bid is higher than the current highest bid

            if order_price > self.highest_bid {
                self.highest_bid = order_price;
            }

            // Update the lowest bid if the new bid is lower than the current lowest bid

            if order_price < self.lowest_bid {
                self.lowest_bid = order_price;
            }

            // Return the order receipt NFT

            receipt
        }

        pub fn fill_bid(&mut self, nfts: NonFungibleBucket) -> (Vec<Bucket>, Option<Vec<Bucket>>) {
            let mut payment_to_seller: Vec<Bucket> = vec![];

            assert!(
                nfts.non_fungible_local_ids().len() > 0,
                "[Fill Bid] : NFT amount must be greater than 0"
            );

            assert!(
                nfts.resource_address() == self.collection,
                "[Fill Bid] : NFTs must be from the same collection"
            );

            assert!(self.highest_bid != dec!(0), "[Fill Bid] : No bids to fill");

            let nft_bucket_vec_holder: Vec<Bucket> = vec![nfts.into()];
            let mut nft_bucket_vec_holder = Some(nft_bucket_vec_holder);
            let mut keys_to_remove = Vec::new();

            for (price_level, _value, next_key) in self
                .price_levels
                .range_back(self.highest_bid..self.lowest_bid)
            {
                // get the order vector for the current price level
                let order_vec = {
                    let orderline = self.orderbook_lines.get(&price_level).unwrap();
                    orderline.level_orders.clone()
                };

                // loop through the potential_bids_to_fill vector, fill the orders - if nft amount has not been filled, then move to the next bid
                // or if it had been filled then continue with the rest of the function

                // If all NFTs have been filled, break out of the price level loop as well
                if nft_bucket_vec_holder.is_none() {
                    self.highest_bid = price_level;
                    break;
                }

                for find_order in &order_vec {
                    let mut nft_bucket_vec_holder_inner = nft_bucket_vec_holder.take().unwrap();

                    let order_id = find_order.clone();

                    let mut nfts = nft_bucket_vec_holder_inner.pop().unwrap();

                    let order = self.orders.get(&order_id).unwrap().clone();

                    let order_qty = order.order_qty;

                    let nft_qty = nfts.amount();

                    assert!(
                        nft_qty > dec!(0),
                        "[Fill Order Partial or Full] : NFT amount must be greater than 0"
                    );

                    match nft_qty {
                        qty if qty == order_qty => {
                            let mut bid_value: Vec<Bucket> = vec![];
                            {
                                let mut vault = self.bid_vaults.get_mut(&order_id).unwrap();
                                bid_value.push(vault.take_all());
                            }
                            let nft_order_fill = nfts.take(order_qty);

                            // place nfts into filled orders vault
                            let filled_order_vault = Vault::with_bucket(nft_order_fill.into());
                            {
                                self.nft_vaults.insert(order_id.clone(), filled_order_vault);
                            }

                            // update nft receipt order status to filled
                            {
                                let mut order = self.orders.get_mut(&order_id).unwrap();
                                order.order_status = OrderStatus::FILLED;
                            }

                            // remove order from bids key value store
                            {
                                self.orders.remove(&order_id);
                            }

                            // remove the order_id from the vector of nonfungibleglobalids in the orderbook line
                            {
                                let mut orderline =
                                    self.orderbook_lines.get_mut(&price_level).unwrap();
                                orderline.level_orders.retain(|x| x != &order_id);
                            }

                            payment_to_seller.extend(bid_value);
                            nft_bucket_vec_holder = None;

                            break;
                        }
                        qty if qty > order_qty => {
                            let mut bid_value: Vec<Bucket> = vec![];
                            {
                                let mut vault = self.bid_vaults.get_mut(&order_id).unwrap();
                                bid_value.push(vault.take_all());
                            }
                            let nft_order_fill = nfts.take(order_qty);

                            // place nfts into filled orders vault
                            let filled_order_vault = Vault::with_bucket(nft_order_fill.into());
                            {
                                self.nft_vaults.insert(order_id.clone(), filled_order_vault);
                            }

                            // update nft receipt order status to filled
                            {
                                let mut order = self.orders.get_mut(&order_id).unwrap();
                                order.order_status = OrderStatus::FILLED;
                            }

                            // remove order from bids key value store
                            {
                                self.orders.remove(&order_id);
                            }

                            // remove the order_id from the vector of nonfungibleglobalids in the orderbook line
                            {
                                let mut orderline =
                                    self.orderbook_lines.get_mut(&price_level).unwrap();
                                orderline.level_orders.retain(|x| x != &order_id);
                            }

                            // take the payment
                            payment_to_seller.extend(bid_value);

                            // return left over nfts
                            nft_bucket_vec_holder = Some(vec![nfts]);
                        }
                        qty if (qty < order_qty) && (qty > dec!(0)) => {
                            let mut bid_value: Vec<Bucket> = vec![];

                            {
                                let mut vault = self.bid_vaults.get_mut(&order_id).unwrap();
                                let order_amount_filled = order_qty - nft_qty;
                                let bid_value_to_take =
                                    order_amount_filled.checked_mul(order.order_price).unwrap();
                                bid_value.push(vault.take(bid_value_to_take));
                            }

                            let nft_order_fill = nfts.take(nft_qty);

                            // place nfts into filled orders vault
                            let filled_order_vault = Vault::with_bucket(nft_order_fill.into());
                            {
                                self.nft_vaults.insert(order_id.clone(), filled_order_vault);
                            }

                            // update order status to partial
                            {
                                let mut order = self.orders.get_mut(&order_id).unwrap();
                                order.order_status = OrderStatus::PARTIAL;
                            }
                            payment_to_seller.extend(bid_value);
                            nft_bucket_vec_holder = None;
                            keys_to_remove.push(price_level);
                            break;
                        }
                        _ => panic!("Should not reach here"),
                    }
                }

                if next_key.is_none() {
                    self.highest_bid = dec!(0);
                    self.lowest_bid = dec!(0);
                    break;
                }
            }
            for key in keys_to_remove {
                self.price_levels.remove(&key);
                self.orderbook_lines.remove(&key);
            }
            (payment_to_seller, nft_bucket_vec_holder)
        }

        fn get_order(&self, order_id: NonFungibleGlobalId) -> Order {
            self.orders.get(&order_id).unwrap().clone()
        }

        fn remove_order(&mut self, order_id: NonFungibleGlobalId) {
            self.orders.remove(&order_id);
        }

        // fn remove_bid_ref(&mut self, price: Decimal) {
        //     let bid_ref = self.bids_ref.get(&price).unwrap();
        //     assert!(
        //         bid_ref.clone().len() == 0,
        //         "[Remove Price Level] : Price level must be empty to remove"
        //     );
        //     self.bids_ref.remove(&price);
        // }

        // fn remove_price_level(&mut self, price: Decimal) {
        //     let price_levels = self.price_levels.clone();
        //     let price_levels = price_levels
        //         .iter()
        //         .filter(|p| *p != &price)
        //         .cloned()
        //         .collect::<Vec<Decimal>>();
        //     self.price_levels = price_levels;
        // }

        // fn get_price_levels(&self) -> Vec<Decimal> {
        //     self.price_levels.clone()
        // }

        // fn insert_bid_ref(&mut self, price: Decimal, order_id: NonFungibleGlobalId) {
        //     if let Some(order_ids) = self.bids_ref.get(&price) {
        //         let mut order_ids = order_ids.clone();
        //         order_ids.push(order_id);
        //         self.bids_ref.insert(price, order_ids);
        //     } else {
        //         self.bids_ref.insert(price, vec![order_id]);
        //     }
        // }

        // fn insert_price_level(&mut self, price: Decimal) {
        //     let price_levels = self.price_levels.clone();
        //     let mut price_levels = price_levels.clone();
        //     price_levels.push(price);
        //     self.price_levels = price_levels;
        // }

        // fn get_bid_vault(
        //     &self,
        //     order_id: NonFungibleGlobalId,
        // ) -> KeyValueEntryRef<'_, scrypto::prelude::Vault> {
        //     self.bid_vaults.get(&order_id).unwrap()
        // }

        // fn get_filled_order(
        //     &self,
        //     order_id: NonFungibleGlobalId,
        // ) -> KeyValueEntryRef<'_, scrypto::prelude::Vault> {
        //     self.filled_orders.get(&order_id).unwrap()
        // }

        fn fill_order_partial_or_full(
            &mut self,
            order_id: NonFungibleGlobalId,
            mut seller_nfts: Vec<Bucket>,
        ) -> (Option<Bucket>, Vec<Bucket>) {
            let mut nfts = seller_nfts.pop().unwrap();

            let order = self.orders.get(&order_id).unwrap().clone();

            let order_qty = order.order_qty;

            let nft_qty = nfts.amount();

            assert!(
                nft_qty > dec!(0),
                "[Fill Order Partial or Full] : NFT amount must be greater than 0"
            );

            match nft_qty {
                qty if qty > order_qty => {
                    let mut bid_value: Vec<Bucket> = vec![];
                    {
                        let mut vault = self.bid_vaults.get_mut(&order_id).unwrap();
                        bid_value.push(vault.take_all());
                    }
                    let nft_order_fill = nfts.take(order_qty);

                    // place nfts into filled orders vault
                    let filled_order_vault = Vault::with_bucket(nft_order_fill.into());
                    {
                        self.nft_vaults.insert(order_id.clone(), filled_order_vault);
                    }

                    // update nft receipt order status to filled
                    {
                        self.update_order_to_filled(order_id.clone());
                    }

                    // remove order from bids key value store
                    {
                        self.remove_order(order_id.clone());
                    }

                    (Some(nfts), bid_value)
                }
                qty if (qty < order_qty) && (qty > dec!(0)) => {
                    let mut bid_value: Vec<Bucket> = vec![];

                    {
                        let mut vault = self.bid_vaults.get_mut(&order_id).unwrap();
                        let order_amount_filled = order_qty - nft_qty;
                        let bid_value_to_take =
                            order_amount_filled.checked_mul(order.order_price).unwrap();
                        bid_value.push(vault.take(bid_value_to_take));
                    }

                    let nft_order_fill = nfts.take(nft_qty);

                    // place nfts into filled orders vault
                    let filled_order_vault = Vault::with_bucket(nft_order_fill.into());
                    {
                        self.nft_vaults.insert(order_id.clone(), filled_order_vault);
                    }

                    // update order status to partial
                    {
                        self.update_order_to_partial(order_id.clone());
                    }

                    (None, bid_value)
                }
                _ => {
                    return (None, vec![]);
                }
            }
        }

        fn update_order_to_filled(&mut self, order_id: NonFungibleGlobalId) {
            let mut order = self.orders.get_mut(&order_id).unwrap();
            order.order_status = OrderStatus::FILLED;
        }

        fn update_order_to_partial(&mut self, order_id: NonFungibleGlobalId) {
            let mut order = self.orders.get_mut(&order_id).unwrap();
            order.order_status = OrderStatus::PARTIAL;
        }
    }
}
