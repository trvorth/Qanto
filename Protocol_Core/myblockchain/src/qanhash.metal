#include <metal_stdlib>
using namespace metal;

// Qanhash constants
constant uint MIX_BYTES = 128;
constant uint HASH_BYTES = 32;

// Keccak-f[1600] round constants
constant ulong keccak_round_constants[24] = {
    0x0000000000000001UL, 0x0000000000008082UL, 0x800000000000808aUL,
    0x8000000080008000UL, 0x000000000000808bUL, 0x0000000080000001UL,
    0x8000000080008081UL, 0x8000000000008009UL, 0x000000000000008aUL,
    0x0000000000000088UL, 0x0000000080008009UL, 0x000000008000000aUL,
    0x000000008000808bUL, 0x800000000000008bUL, 0x8000000000008089UL,
    0x8000000000008003UL, 0x8000000000008002UL, 0x8000000000000080UL,
    0x000000000000800aUL, 0x800000008000000aUL, 0x8000000080008081UL,
    0x8000000000008080UL, 0x0000000080000001UL, 0x8000000080008008UL
};

// Rotation offsets for Keccak
constant uint keccak_rotations[24] = {
    1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44
};

// Pi lane mapping for Keccak
constant uint keccak_pi[24] = {
    10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1
};

// Rotate left function
inline ulong rotl64(ulong x, uint n) {
    return (x << n) | (x >> (64 - n));
}

// Keccak-f[1600] permutation
void keccak_f(thread ulong state[25]) {
    for (uint round = 0; round < 24; round++) {
        // Theta step
        ulong C[5];
        for (uint x = 0; x < 5; x++) {
            C[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        
        ulong D[5];
        for (uint x = 0; x < 5; x++) {
            D[x] = C[(x + 4) % 5] ^ rotl64(C[(x + 1) % 5], 1);
        }
        
        for (uint x = 0; x < 5; x++) {
            for (uint y = 0; y < 5; y++) {
                state[y * 5 + x] ^= D[x];
            }
        }
        
        // Rho and Pi steps
        ulong current = state[1];
        for (uint t = 0; t < 24; t++) {
            uint index = keccak_pi[t];
            ulong temp = state[index];
            state[index] = rotl64(current, keccak_rotations[t]);
            current = temp;
        }
        
        // Chi step
        for (uint y = 0; y < 5; y++) {
            ulong temp[5];
            for (uint x = 0; x < 5; x++) {
                temp[x] = state[y * 5 + x];
            }
            for (uint x = 0; x < 5; x++) {
                state[y * 5 + x] = temp[x] ^ ((~temp[(x + 1) % 5]) & temp[(x + 2) % 5]);
            }
        }
        
        // Iota step
        state[0] ^= keccak_round_constants[round];
    }
}

// Qanhash mix function optimized for Metal
void qanhash_mix(thread uchar mix[MIX_BYTES], 
                 constant uchar dag[][MIX_BYTES],
                 ulong dag_len_mask,
                 thread ulong state[25]) {
    // Initialize mix with header data
    for (uint i = 0; i < MIX_BYTES / 8; i++) {
        ((thread ulong*)mix)[i] = state[i % 25];
    }
    
    // Perform mixing rounds
    for (uint round = 0; round < 64; round++) {
        // Calculate DAG index from mix
        ulong dag_index = 0;
        for (uint i = 0; i < 8; i++) {
            dag_index ^= ((thread ulong*)mix)[i];
        }
        dag_index &= dag_len_mask;
        
        // XOR with DAG data
        for (uint i = 0; i < MIX_BYTES; i++) {
            mix[i] ^= dag[dag_index][i];
        }
        
        // Update state with mix data
        for (uint i = 0; i < MIX_BYTES / 8; i++) {
            state[i % 25] ^= ((thread ulong*)mix)[i];
        }
        
        // Apply Keccak permutation every 8 rounds
        if ((round & 7) == 7) {
            keccak_f(state);
        }
    }
}

// Main Qanhash kernel
kernel void qanhash_kernel(
    constant uchar* header [[buffer(0)]],
    constant ulong& start_nonce [[buffer(1)]],
    constant uchar dag[][MIX_BYTES] [[buffer(2)]],
    constant ulong& dag_len_mask [[buffer(3)]],
    constant uchar* target [[buffer(4)]],
    device ulong* result_nonce [[buffer(5)]],
    device uchar* result_hash [[buffer(6)]],
    device uint* result_found [[buffer(7)]],
    uint gid [[thread_position_in_grid]]
) {
    // Early exit if solution already found
    if (*result_found != 0) {
        return;
    }
    
    ulong nonce = start_nonce + gid;
    
    // Initialize Keccak state with header and nonce
    ulong state[25] = {0};
    
    // Copy header (32 bytes) into state
    for (uint i = 0; i < 4; i++) {
        state[i] = ((constant ulong*)header)[i];
    }
    
    // Add nonce
    state[4] = nonce;
    
    // Apply initial Keccak permutation
    keccak_f(state);
    
    // Perform Qanhash mixing
    uchar mix[MIX_BYTES];
    qanhash_mix(mix, dag, dag_len_mask, state);
    
    // Final Keccak permutation
    keccak_f(state);
    
    // Extract final hash (first 32 bytes of state)
    uchar final_hash[32];
    for (uint i = 0; i < 4; i++) {
        ((thread ulong*)final_hash)[i] = state[i];
    }
    
    // Check if hash meets target (little-endian comparison)
    bool valid = true;
    for (int i = 31; i >= 0; i--) {
        if (final_hash[i] > target[i]) {
            valid = false;
            break;
        } else if (final_hash[i] < target[i]) {
            break;
        }
    }
    
    // If valid solution found, atomically claim it
    if (valid) {
        uint expected = 0;
        if (atomic_compare_exchange_weak_explicit(
            (device atomic_uint*)result_found,
            &expected,
            1,
            memory_order_relaxed,
            memory_order_relaxed)) {
            
            // We won the race, store our result
            *result_nonce = nonce;
            for (uint i = 0; i < 32; i++) {
                result_hash[i] = final_hash[i];
            }
        }
    }
}