#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::{fmt::Debug, vec::Vec};
use sp_std::prelude::*;
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, ensure,
    dispatch::DispatchResultWithPostInfo,
    traits::{Get, EnsureOrigin},
	weights::Weight,
};
use frame_system::ensure_signed;
use sp_runtime::{
    DispatchError, RuntimeDebug,
};
use codec::{Encode, Decode};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod default_weights;

/// An identifier for a single name registrar/identity verification service.
pub type RegistrarIndex = u32;

pub trait WeightInfo {
	fn add_registrar(r: u32, ) -> Weight;
	fn register(r: u32, ) -> Weight;
	fn unregister() -> Weight;
	fn provide_judgement(r: u32, ) -> Weight;
    fn kill_username() -> Weight;
}

#[derive(Copy, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug)]
pub enum Judgement {
    Requested,
    Approved,
}

/// Information concerning the username registration of the controller of an account.
///
/// NOTE: This is stored separately primarily to facilitate the addition of extra fields in a
/// backwards compatible way through a specialized `Decode` impl.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug)]
pub struct Registration<
    AccountId: Encode + Decode + Clone + Debug + Eq + PartialEq
> {
	/// Judgements from the registrars on this identity. Stored ordered by `RegistrarIndex`. There
	/// may be only a single judgement from each registrar.
	pub judgements: Vec<(RegistrarIndex, Judgement)>,

	/// Account Id.
	pub account_id: AccountId,
}

pub trait Trait: frame_system::Trait {
	/// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;

	/// Maxmimum number of registrars allowed in the system. Needed to bound the complexity
	/// of, e.g., updating judgements.
	type MaxRegistrars: Get<u32>;

    /// Minimum username length
    type MinUsernameLength: Get<u32>;

    /// Maximum username length
    type MaxUsernameLength: Get<u32>;

	/// The origin which may forcibly set or remove a name. Root can always do this.
	type ForceOrigin: EnsureOrigin<Self::Origin>;

    /// The origin which may add or remove registrars. Root can always do this.
	type RegistrarOrigin: EnsureOrigin<Self::Origin>;

	/// Weight information for extrinsics in this pallet.
	type WeightInfo: WeightInfo;
}


decl_storage! {
    trait Store for Module<T: Trait> as ValidatorRegistry {
        pub RegistrationOf get(fn registration_of): map hasher(twox_64_concat) Vec<u8> => Option<Registration<T::AccountId>>;

		pub Registrars get(fn registrars): Vec<Option<T::AccountId>>;
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
    {
        UsernameRegistered(AccountId),
        UsernameUnregistered(AccountId),
        UsernameKilled(AccountId),
		JudgementRequested(AccountId, RegistrarIndex),
        JudgementGiven(AccountId, RegistrarIndex),
		RegistrarAdded(RegistrarIndex),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        TooManyRegistrars,
        EmptyIndex,
        InvalidIndex,
        UsernameIsVeryLong,
        UsernameIsVeryShort,
        UsernameAlreadyRegistered,
        UnregisterForbidden,
        UsernameNotFound,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		/// Maxmimum number of registrars allowed in the system. Needed to bound the complexity
		/// of, e.g., updating judgements.
		const MaxRegistrars: u32 = T::MaxRegistrars::get();

        const MinUsernameLength: u32 = T::MinUsernameLength::get();
        const MaxUsernameLength: u32 = T::MaxUsernameLength::get();

        type Error = Error<T>;

        fn deposit_event() = default;
        
		/// Add a registrar to the system.
		///
		/// The dispatch origin for this call must be `T::RegistrarOrigin`.
		///
		/// - `account`: the account of the registrar.
		///
		/// Emits `RegistrarAdded` if successful.
		///
		/// # <weight>
		/// - `O(R)` where `R` registrar-count (governance-bounded and code-bounded).
		/// - One storage mutation (codec `O(R)`).
		/// - One event.
		/// # </weight>
		#[weight = T::WeightInfo::add_registrar(T::MaxRegistrars::get()) ]
        fn add_registrar(origin, account: T::AccountId) -> DispatchResultWithPostInfo {
			T::RegistrarOrigin::ensure_origin(origin)?;

			let (i, registrar_count) = <Registrars<T>>::try_mutate(
				|registrars| -> Result<(RegistrarIndex, usize), DispatchError> {
					ensure!(registrars.len() < T::MaxRegistrars::get() as usize, Error::<T>::TooManyRegistrars);
					registrars.push(Some(account));
					Ok(((registrars.len() - 1) as RegistrarIndex, registrars.len()))
				}
			)?;

			Self::deposit_event(RawEvent::RegistrarAdded(i));

			Ok(Some(T::WeightInfo::add_registrar(registrar_count as u32)).into())
		}

        /// Register an username and request registration
		///
		/// The dispatch origin for this call must be _Signed_.
		///
		/// - `username`: username.
        /// - `reg_index`: registrar index.
		///
		/// Emits `UsernameRegistered` and `JudgementRequested` if successful.
		///
        /// # <weight>
		/// - `O(R)` where `R` registrar-count (governance-bounded and code-bounded).
		/// - One storage mutation (codec `O(R)`).
		/// - One event.
		/// # </weight>
		#[weight =  T::WeightInfo::register(T::MaxRegistrars::get().into())]
		fn register(origin, username: Vec<u8>, #[compact] reg_index: RegistrarIndex) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

            ensure!(username.len() >= T::MinUsernameLength::get() as usize, Error::<T>::UsernameIsVeryShort);
            ensure!(username.len() <= T::MaxUsernameLength::get() as usize, Error::<T>::UsernameIsVeryLong);
            ensure!(!<RegistrationOf<T>>::contains_key(&username), Error::<T>::UsernameAlreadyRegistered);

			let registrars = <Registrars<T>>::get();
            let _registrar = registrars.get(reg_index as usize).and_then(Option::as_ref)
				.ok_or(Error::<T>::EmptyIndex)?;

            let item = (reg_index, Judgement::Requested);
            <RegistrationOf<T>>::insert(&username, Registration { judgements: vec![item], account_id: sender.clone() });

			Self::deposit_event(RawEvent::UsernameRegistered(sender.clone()));
			Self::deposit_event(RawEvent::JudgementRequested(sender, reg_index));

            Ok(Some(T::WeightInfo::register(registrars.len() as u32)).into())
		}

        /// Unregister an username
		///
		/// The dispatch origin for this call must be _Signed_ and the sender must have a registered
		/// identity.
        ///
		/// - `username`: username.
		///
		/// Emits `UsernameUnregistered` if successful.
		///
		/// # <weight>
        /// # </weight>
		#[weight = T::WeightInfo::unregister()]
		fn unregister(origin, username: Vec<u8>) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

            ensure!(username.len() >= T::MinUsernameLength::get() as usize, Error::<T>::UsernameIsVeryShort);
            ensure!(username.len() <= T::MaxUsernameLength::get() as usize, Error::<T>::UsernameIsVeryLong);

            if let Some(registration) = <RegistrationOf<T>>::get(&username) {
                if registration.account_id == sender {
                    <RegistrationOf<T>>::remove(&username)
                } else {
                    Err(Error::<T>::UnregisterForbidden)?
                }
            } else {
                Err(Error::<T>::UsernameNotFound)?
            }
		
			Self::deposit_event(RawEvent::UsernameUnregistered(sender));

			Ok(Some(T::WeightInfo::unregister()).into())
		}

        /// Remove username
		///
		/// The dispatch origin for this call must match `T::ForceOrigin`.
		///
		/// - `username`: username.
		///
		/// Emits `UsernameKilled` if successful.
		///
		/// # <weight>
		/// # </weight>
		#[weight = T::WeightInfo::kill_username()]
		fn kill_username(origin, username: Vec<u8>) -> DispatchResultWithPostInfo {
			T::ForceOrigin::ensure_origin(origin)?;

            ensure!(username.len() >= T::MinUsernameLength::get() as usize, Error::<T>::UsernameIsVeryShort);
            ensure!(username.len() <= T::MaxUsernameLength::get() as usize, Error::<T>::UsernameIsVeryLong);
            let registration = <RegistrationOf<T>>::take(&username).ok_or(Error::<T>::UsernameNotFound)?;
            
			Self::deposit_event(RawEvent::UsernameKilled(registration.account_id));

			Ok(Some(T::WeightInfo::kill_username()).into())
		}

		/// Provide a judgement for an username.
		///
		/// The dispatch origin for this call must be _Signed_ and the sender must be the account
		/// of the registrar whose index is `reg_index`.
		///
		/// - `reg_index`: the index of the registrar whose judgement is being made.
		/// - `username`: username
		/// - `judgement`: the judgement of the registrar of index `reg_index` about `target`.
		///
		/// Emits `JudgementGiven` if successful.
		///
        /// # <weight>
		/// - `O(R)` where `R` registrar-count (governance-bounded and code-bounded).
		/// - One storage mutation (codec `O(R)`).
		/// - One event.
		/// # </weight>
		#[weight = T::WeightInfo::provide_judgement(T::MaxRegistrars::get().into())]
		fn provide_judgement(origin,
			#[compact] reg_index: RegistrarIndex,
            username: Vec<u8>,
			judgement: Judgement,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

            ensure!(username.len() >= T::MinUsernameLength::get() as usize, Error::<T>::UsernameIsVeryShort);
            ensure!(username.len() <= T::MaxUsernameLength::get() as usize, Error::<T>::UsernameIsVeryLong);
            let mut registration = <RegistrationOf<T>>::get(&username).ok_or(Error::<T>::UsernameNotFound)?;

            let registrars = <Registrars<T>>::get();
            registrars
				.get(reg_index as usize)
				.and_then(Option::as_ref)
				.and_then(|account| if *account == sender { Some(account) } else { None })
				.ok_or(Error::<T>::InvalidIndex)?;

            let item = (reg_index, judgement);
			match registration.judgements.binary_search_by_key(&reg_index, |x| x.0) {
				Ok(position) => registration.judgements[position] = item,
				Err(position) => registration.judgements.insert(position, item),
			}

            let target = registration.account_id.clone();
			<RegistrationOf<T>>::insert(&username, registration);
			Self::deposit_event(RawEvent::JudgementGiven(target, reg_index));

			Ok(Some(T::WeightInfo::provide_judgement(registrars.len() as u32,)).into())
		}
    }
}
