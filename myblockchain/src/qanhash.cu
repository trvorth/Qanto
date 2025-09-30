#include <cuda_runtime.h>
#include <stdint.h>

// Qanhash constants
#define MIX_BYTES 128
#define HASH_BYTES 32

// Keccak-f[1600] round constants
__constant__ uint64_t keccak_round_constants[24] = {
    0x0000000000000001ULL, 0x0000000000008082ULL, 0x800000000000808aULL,
    0x8000000080008000ULL, 0x000000000000808bULL, 0x0000000080000001ULL,
    0x8000000080008081ULL, 0x8000000000008009ULL, 0x000000000000008aULL,
    0x0000000000000088ULL, 0x000000008000009ULL, 0x000000008000000aULL,
    0x000000008000808bULL, 0x800000000000008bULL, 0x8000000000008089ULL,
    0x8000000000008003ULL, 0x8000000000008002ULL, 0x8000000000000080ULL,
    0x000000000000800aULL, 0x800000008000000aULL, 0x8000000080008081ULL,
    0x8000000000008080ULL, 0x0000000080000001ULL, 0x8000000080008008ULL
};

// Rotation offsets for Keccak
__constant__ uint32_t keccak_rotations[24] = {
    1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44
};

// Pi lane mapping for Keccak
__constant__ uint32_t keccak_pi[24] = {
    10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1
};

// Rotate left function
__device__ __forceinline__ uint64_t rotl64(uint64_t x, uint32_t n) {
    return (x << n) | (x >> (64 - n));
}

// Keccak-f[1600] permutation
__device__ void keccak_f(uint64_t state[25]) {
    for (uint32_t round = 0; round < 24; round++) {
        // Theta step
        uint64_t C[5];
        for (uint32_t x = 0; x < 5; x++) {
            C[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        
        uint64_t D[5];
        for (uint32_t x = 0; x < 5; x++) {
            D[x] = C[(x + 4) % 5] ^ rotl64(C[(x + 1) % 5], 1);
        }
        
        for (uint32_t x = 0; x < 5; x++) {
            for (uint32_t y = 0; y < 5; y++) {
                state[y * 5 + x] ^= D[x];
            }
        }
        
        // Rho and Pi steps
        uint64_t current = state[1];
        for (uint32_t t = 0; t < 24; t++) {
            uint32_t index = keccak_pi[t];
            uint64_t temp = state[index];
            state[index] = rotl64(current, keccak_rotations[t]);
            current = temp;
        }
        
        // Chi step
        for (uint32_t y = 0; y < 5; y++) {
            uint64_t temp[5];
            for (uint32_t x = 0; x < 5; x++) {
                temp[x] = state[y * 5 + x];
            }
            for (uint32_t x = 0; x < 5; x++) {
                state[y * 5 + x] = temp[x] ^ ((~temp[(x + 1) % 5]) & temp[(x + 2) % 5]);
            }
        }
        
        // Iota step
        state[0] ^= keccak_round_constants[round];
    }
}

// Qanhash mix function optimized for CUDA
__device__ void qanhash_mix(uint8_t mix[MIX_BYTES], 
                           const uint8_t* dag,
                           uint64_t dag_len_mask,
                           uint64_t state[25]) {
    // Initialize mix with header data
    for (uint32_t i = 0; i < MIX_BYTES / 8; i++) {
        ((uint64_t*)mix)[i] = state[i % 25];
    }
    
    // Perform mixing rounds
    for (uint32_t round = 0; round < 64; round++) {
        // Calculate DAG index from mix
        uint64_t dag_index = 0;
        for (uint32_t i = 0; i < 8; i++) {
            dag_index ^= ((uint64_t*)mix)[i];
        }
        dag_index &= dag_len_mask;
        
        // XOR with DAG data
        const uint8_t* dag_entry = dag + (dag_index * MIX_BYTES);
        for (uint32_t i = 0; i < MIX_BYTES; i++) {
            mix[i] ^= dag_entry[i];
        }
        
        // Update state with mix data
        for (uint32_t i = 0; i < MIX_BYTES / 8; i++) {
            state[i % 25] ^= ((uint64_t*)mix)[i];
        }
        
        // Apply Keccak permutation every 8 rounds
        if ((round & 7) == 7) {
            keccak_f(state);
        }
    }
}

// Main Qanhash CUDA kernel
extern "C" __global__ void qanhash_kernel(
    const uint8_t* header,
    uint64_t start_nonce,
    const uint8_t* dag,
    uint64_t dag_len_mask,
    const uint8_t* target,
    uint64_t* result_nonce,
    uint8_t* result_hash,
    uint32_t* result_found
) {
    uint32_t gid = blockIdx.x * blockDim.x + threadIdx.x;
    
    // Early exit if solution already found
    if (*result_found != 0) {
        return;
    }
    
    uint64_t nonce = start_nonce + gid;
    
    // Initialize Keccak state with header and nonce
    uint64_t state[25] = {0};
    
    // Copy header (32 bytes) into state
    for (uint32_t i = 0; i < 4; i++) {
        state[i] = ((const uint64_t*)header)[i];
    }
    
    // Add nonce
    state[4] = nonce;
    
    // Apply initial Keccak permutation
    keccak_f(state);
    
    // Perform Qanhash mixing
    uint8_t mix[MIX_BYTES];
    qanhash_mix(mix, dag, dag_len_mask, state);
    
    // Final Keccak permutation
    keccak_f(state);
    
    // Extract final hash (first 32 bytes of state)
    uint8_t final_hash[32];
    for (uint32_t i = 0; i < 4; i++) {
        ((uint64_t*)final_hash)[i] = state[i];
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
        uint32_t expected = 0;
        if (atomicCAS(result_found, expected, 1) == expected) {
            // We won the race, store our result
            *result_nonce = nonce;
            for (uint32_t i = 0; i < 32; i++) {
                result_hash[i] = final_hash[i];
            }
        }
    }
}