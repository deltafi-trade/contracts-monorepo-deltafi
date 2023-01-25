use {
    arbitrary::Arbitrary,
    honggfuzz::fuzz,
    deltafi_swap::{
        instruction::{FarmDepositData, FarmWithdrawData},
    },
    deltafi_swap_fuzz::{
        native_farm:: NativeFarm,
    },
};

use solana_program::entrypoint::ProgramResult;

#[derive(Debug, Arbitrary, Clone)]
struct FarmFuzzData {
    pool_reserved_amount: u64,
    instructions: Vec<FarmFuzzInstruction>,
}

#[derive(Debug, Arbitrary, Clone)]
enum FarmFuzzInstruction {
    Deposit {
        instruction_data: FarmDepositData,
    },
    Withdraw {
        instruction_data: FarmWithdrawData,
    },
    Refresh,
}

const MAX_POOL_RESERVED_AMOUNT: u64 = 100_000_000_000;

fn main() {
    loop {
        fuzz!(|fuzz_data: FarmFuzzData| { run_farm_fuzz(fuzz_data) });
    }
}

fn run_farm_fuzz(fuzz_data: FarmFuzzData) {

    /*  Create and initialize a simulated environment to fuzz Farm Instructions.
        Create accounts that are to be used with FarmInstructions,
        and initialize Farm Pool
    */
    let mut pool_reserved_amount = fuzz_data.pool_reserved_amount;
    if pool_reserved_amount > MAX_POOL_RESERVED_AMOUNT {
        pool_reserved_amount = MAX_POOL_RESERVED_AMOUNT;
    }
    let mut native_farm = NativeFarm::new(pool_reserved_amount);
 
    let original_amount = native_farm.get_deposited_amount().unwrap();
    let mut deposited_amount: u64 = 0;
    let mut withdrawn_amount: u64 = 0;

    /* run fuzz instructions with fuzzer inputed data */
    for fuzz_instruction in fuzz_data.instructions {

        let result = run_fuzz_instruction(
            fuzz_instruction.clone(),
            &mut native_farm,
        );

        match fuzz_instruction {
            FarmFuzzInstruction::Deposit {instruction_data} => {
                match result {
                    Ok(()) => {
                        println!("Deposit Ok");
                        deposited_amount = deposited_amount.checked_add(instruction_data.amount).unwrap();
                        
                    }
                    Err(e) => {
                        println!("{:?}", e);
                        println!("Deposit Ok");
                    }                        
                };
                
            }
            FarmFuzzInstruction::Withdraw {instruction_data} => {
                match result {
                    Ok(()) => {
                        println!("Withdraw Ok");
                        withdrawn_amount = withdrawn_amount.checked_add(instruction_data.amount).unwrap();
                    }
                    Err(e) => {
                        println!("{:?}", e);
                    }                        
                };
            }
            FarmFuzzInstruction::Refresh => {
            }
    
        };
    }

    /* verify amount */
    let updated_amount = native_farm.get_deposited_amount().unwrap();
    let mut latest_amount: u64 = 0;

    latest_amount = latest_amount.checked_add(original_amount).unwrap();
    latest_amount = latest_amount.checked_add(deposited_amount).unwrap();
    latest_amount = latest_amount.checked_sub(withdrawn_amount).unwrap();
    assert_eq!(
        updated_amount,
        latest_amount
    );
    
}

fn run_fuzz_instruction(
    fuzz_instruction: FarmFuzzInstruction,
    native_farm: &mut NativeFarm,
) -> ProgramResult {
    let result = match fuzz_instruction {
        FarmFuzzInstruction::Deposit {instruction_data} => {
            native_farm.run_farm_deposit(instruction_data)
        }
        FarmFuzzInstruction::Withdraw {instruction_data} => {
            native_farm.run_farm_withdraw(instruction_data)
        }
        FarmFuzzInstruction::Refresh => {
            native_farm.run_farm_refresh()
        }

    };
    result
}
