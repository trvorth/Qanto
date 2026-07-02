import { configureStore, createAsyncThunk, createSlice, type PayloadAction } from "@reduxjs/toolkit";
import { invoke } from "@tauri-apps/api/core";

export type WalletBalanceState = {
  available: string;
  pending: string;
  total: string;
};

export type WalletBalanceBaseUnitsState = {
  available: string;
  pending: string;
  total: string;
};

export type WalletState = {
  currentAddress: string | null;
  balance: WalletBalanceState;
  balanceBaseUnits: WalletBalanceBaseUnitsState;
  balanceStatus: "idle" | "loading" | "succeeded" | "failed";
  balanceError: string | null;
};

const initialState: WalletState = {
  currentAddress: null,
  balance: {
    available: "0.000000000 QNTO",
    pending: "0.000000000 QNTO",
    total: "0.000000000 QNTO",
  },
  balanceBaseUnits: {
    available: "0",
    pending: "0",
    total: "0",
  },
  balanceStatus: "idle",
  balanceError: null,
};

type GetWalletBalanceResponse = {
  balance: string;
  unconfirmed_balance: string;
};

function formatBaseUnitsToQNTO(baseUnits: string): string {
  const smallestUnits = 1_000_000_000n;
  const raw = BigInt(baseUnits);
  const intPart = raw / smallestUnits;
  const frac9 = raw % smallestUnits;

  return `${intPart.toString()}.${frac9.toString().padStart(9, "0")} QNTO`;
}

export const refreshBalance = createAsyncThunk<
  { display: WalletBalanceState; baseUnits: WalletBalanceBaseUnitsState },
  { address?: string } | undefined,
  { state: RootState; rejectValue: string }
>("wallet/refreshBalance", async (arg, thunkApi) => {
  const address = arg?.address ?? thunkApi.getState().wallet.currentAddress;
  if (!address) {
    return thunkApi.rejectWithValue("no wallet address selected");
  }

  try {
    const resp = await invoke<GetWalletBalanceResponse>("get_wallet_balance", { address });
    const confirmed = BigInt(resp.balance);
    const pending = BigInt(resp.unconfirmed_balance);
    const total = confirmed + pending;

    return {
      display: {
        available: formatBaseUnitsToQNTO(confirmed.toString()),
        pending: formatBaseUnitsToQNTO(pending.toString()),
        total: formatBaseUnitsToQNTO(total.toString()),
      },
      baseUnits: {
        available: confirmed.toString(),
        pending: pending.toString(),
        total: total.toString(),
      },
    };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return thunkApi.rejectWithValue(message);
  }
});

const walletSlice = createSlice({
  name: "wallet",
  initialState,
  reducers: {
    setCurrentAddress(state, action: PayloadAction<string | null>) {
      state.currentAddress = action.payload;
      state.balanceError = null;
      state.balanceStatus = "idle";
    },
  },
  extraReducers: (builder) => {
    builder
      .addCase(refreshBalance.pending, (state) => {
        state.balanceStatus = "loading";
        state.balanceError = null;
      })
      .addCase(refreshBalance.fulfilled, (state, action) => {
        state.balanceStatus = "succeeded";
        state.balance = action.payload.display;
        state.balanceBaseUnits = action.payload.baseUnits;
      })
      .addCase(refreshBalance.rejected, (state, action) => {
        state.balanceStatus = "failed";
        state.balanceError = action.payload ?? "balance refresh failed";
      });
  },
});

export const { setCurrentAddress } = walletSlice.actions;

export const store = configureStore({
  reducer: {
    wallet: walletSlice.reducer,
  },
});

export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
