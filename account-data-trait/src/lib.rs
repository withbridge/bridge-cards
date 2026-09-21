use anchor_lang::Discriminator;
use borsh::BorshSerialize;

pub trait AccountData: BorshSerialize + Discriminator {
    fn account_data(&self) -> Vec<u8> {
        let mut data = vec![];
        data.extend_from_slice(Self::DISCRIMINATOR);
        borsh::to_vec(self).unwrap().iter().for_each(|b| data.push(*b));
        data
    }
}
