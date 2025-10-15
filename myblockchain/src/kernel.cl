/**
 * @file kernel.cl
 * @author Trevor (trvorth@qanto.org)
 * @brief High-performance OpenCL kernel for the Qanhash algorithm.
 * @version 0.1.0
 *
 * @copyright Copyright (c) 2025
 *
 * This kernel is optimized for high-throughput GPU hashing. It uses ulong16
 * vector types to process 128 bytes of the mix state per operation, maximizing
 * the parallel processing power of modern GPUs. It also features an atomic
 * flag to ensure only the first valid solution is returned, minimizing wasted work.
 */

#pragma OPENCL EXTENSION cl_khr_int64_base_atomics : enable

/**
 * @brief A highly optimized mixing function for the Qanhash algorithm.
 *
 * This function uses vector types (ulong16) and simple, fast GPU operations
 * like bitwise shifts and XORs to achieve maximum performance.
 *
 * @param val The current mix state vector.
 * @param dag_val The vector read from the Q-DAG.
 * @return The next mix state vector.
 */
inline ulong16 mix_round_optimized(ulong16 val, ulong16 dag_val) {
    // Use bitwise shifts and XORs, which are extremely fast on GPUs.
    val = (val << 3) | (val >> 61); // Bitwise rotation
    val ^= dag_val;
    val = (val << 23) | (val >> 41); // Another rotation
    val ^= val >> 17; // Bitwise shift and XOR
    return val;
}

/**
 * @brief The main Qanhash kernel for mining.
 *
 * Each work-item (thread) processes a single nonce. It computes the Qanhash
 * and checks if it meets the required difficulty target.
 *
 * @param header_hash The 32-byte hash of the block header.
 * @param start_nonce The starting nonce for this batch.
 * @param q_dag A pointer to the pre-generated Q-DAG in global memory.
 * @param dag_len_mask A mask for quick modulus operations on the DAG size (must be power of 2).
 * @param target The 32-byte difficulty target.
 * @param result_gid A global atomic flag to store the winning thread's ID. Initialized to 0xFFFFFFFF.
 * @param result_hash A buffer to store the resulting hash from the winning thread.
 */
__kernel void qanhash_kernel(
    __global const uchar* header_hash,
    ulong start_nonce,
    __global const ulong16* q_dag,
    ulong dag_len_mask,
    __global const uchar* target,
    __global volatile uint* result_gid,
    __global uchar* result_hash
) {
    uint gid = get_global_id(0);

    // --- Early Exit Optimization ---
    // If another thread has already found a solution, exit immediately to save power and cycles.
    if (result_gid[0] != 0xFFFFFFFF) {
        return;
    }

    ulong nonce = start_nonce + gid;
    ulong16 mix[2]; // Use two vectors for a 256-byte state

    // --- Initialization ---
    // Load the 32-byte header hash and the current nonce into the mix state vectors.
    // The rest of the state is initialized to zero.
    mix[0] = (ulong16)(
        *((__global ulong*) (header_hash)),
        *((__global ulong*) (header_hash + 8)),
        *((__global ulong*) (header_hash + 16)),
        *((__global ulong*) (header_hash + 24)),
        nonce, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
    );
    mix[1] = (ulong16)(0);

    // --- Main Hashing Loop ---
    // The loop is unrolled to improve instruction pipelining on the GPU.
    #pragma unroll 8
    for (int i = 0; i < 32; ++i) {
        // Determine DAG lookup indices from the current mix state.
        ulong p_index = mix[0].s0 + mix[1].s0;
        ulong lookup1 = p_index & dag_len_mask;
        ulong lookup2 = (p_index + 1) & dag_len_mask;

        // Perform the mixing round using the optimized function.
        mix[0] = mix_round_optimized(mix[0], q_dag[lookup1]);
        mix[1] = mix_round_optimized(mix[1], q_dag[lookup2]);
    }

    // --- Solution Check ---
    // Cast the final mix state to a byte pointer for comparison with the target.
    __private uchar* final_hash_ptr = (__private uchar*)mix;

    // Compare the resulting hash against the target, byte by byte, from most significant to least.
    for (int i = 0; i < 32; ++i) {
        if (final_hash_ptr[i] < target[i]) {
            // Found a valid solution. Atomically try to claim the result.
            if (atom_cmpxchg(result_gid, 0xFFFFFFFF, gid) == 0xFFFFFFFF) {
                // This thread is the first to find a solution. Copy the hash to the output buffer.
                for(int j = 0; j < 32; ++j) {
                    result_hash[j] = final_hash_ptr[j];
                }
            }
            return; // Exit after finding a solution.
        }
        if (final_hash_ptr[i] > target[i]) {
            // Hash is greater than the target, so it's not a solution. Exit.
            return;
        }
    }

    // This part is reached only if the hash is exactly equal to the target.
    if (atom_cmpxchg(result_gid, 0xFFFFFFFF, gid) == 0xFFFFFFFF) {
        for(int j = 0; j < 32; ++j) {
            result_hash[j] = final_hash_ptr[j];
        }
    }
}
