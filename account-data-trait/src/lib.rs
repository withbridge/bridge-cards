use anchor_lang::Discriminator;
use borsh::BorshSerialize;

pub trait AccountData: BorshSerialize + Discriminator {
    fn account_data(&self) -> Vec<u8> {
        let mut data = vec![];
        data.extend_from_slice(Self::DISCRIMINATOR);
        data.extend_from_slice(self.try_to_vec().unwrap().as_ref());
        data
    }
}
