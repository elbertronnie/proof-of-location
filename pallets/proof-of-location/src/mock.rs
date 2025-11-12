use crate as pallet_proof_of_location;
use frame_support::{derive_impl, parameter_types};
use sp_runtime::{testing::TestXt, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;
type Extrinsic = TestXt<RuntimeCall, ()>;

#[frame_support::runtime]
mod runtime {
    // The main runtime
    #[runtime::runtime]
    // Runtime Types to be generated
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask,
        RuntimeViewFunction
    )]
    pub struct Test;

    #[runtime::pallet_index(0)]
    pub type System = frame_system::Pallet<Test>;

    #[runtime::pallet_index(1)]
    pub type ProofOfLocation = pallet_proof_of_location::Pallet<Test>;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = sp_runtime::AccountId32;
    type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
}

// Server configuration constants
parameter_types! {
    pub const ServerUrl: &'static [u8] = b"localhost:3000";
    pub const MaxDistanceMeters: u32 = 10;
}

impl pallet_proof_of_location::Config for Test {
    type AuthorityId = pallet_proof_of_location::crypto::TestAuthId;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type ServerUrl = ServerUrl;
    type MaxDistanceMeters = MaxDistanceMeters;
}

impl frame_system::offchain::SigningTypes for Test {
    type Public = <sp_runtime::MultiSignature as sp_runtime::traits::Verify>::Signer;
    type Signature = sp_runtime::MultiSignature;
}

impl<LocalCall> frame_system::offchain::CreateTransactionBase<LocalCall> for Test
where
    RuntimeCall: From<LocalCall>,
{
    type Extrinsic = Extrinsic;
    type RuntimeCall = RuntimeCall;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Test
where
    RuntimeCall: From<LocalCall>,
{
    fn create_signed_transaction<
        C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>,
    >(
        call: RuntimeCall,
        _public: Self::Public,
        _account: <Test as frame_system::Config>::AccountId,
        _nonce: <Test as frame_system::Config>::Nonce,
    ) -> Option<Self::Extrinsic> {
        Some(Extrinsic::new_bare(call))
    }
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap()
        .into()
}
