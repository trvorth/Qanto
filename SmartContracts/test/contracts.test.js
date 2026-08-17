import { expect } from "chai";
import { network } from "hardhat";

const { ethers, networkHelpers } = await network.create();
const { time } = networkHelpers;

// ─── QantoFaucet Tests ────────────────────────────────────────────────────────

describe("QantoFaucet", function () {
  let faucet, owner, alice, bob;

  const MINT_AMOUNT    = ethers.parseEther("100");
  const MAX_SUPPLY     = ethers.parseEther("10000000");
  const CLAIM_INTERVAL = 24 * 60 * 60; // seconds
  const MAX_DAILY_MINT = ethers.parseEther("500000");

  beforeEach(async function () {
    [owner, alice, bob] = await ethers.getSigners();
    const Faucet = await ethers.getContractFactory("QantoFaucet");
    faucet = await Faucet.deploy();
    await faucet.waitForDeployment();
  });

  // ── Deployment checks ──────────────────────────────────────────────────────

  it("deploys with zero total supply", async function () {
    expect(await faucet.totalSupply()).to.equal(0n);
  });

  it("has correct constants", async function () {
    expect(await faucet.MINT_AMOUNT()).to.equal(MINT_AMOUNT);
    expect(await faucet.MAX_SUPPLY()).to.equal(MAX_SUPPLY);
    expect(await faucet.CLAIM_INTERVAL()).to.equal(CLAIM_INTERVAL);
    expect(await faucet.MAX_DAILY_MINT()).to.equal(MAX_DAILY_MINT);
  });

  it("has correct name and symbol", async function () {
    expect(await faucet.name()).to.equal("Qanto Genesis Token");
    expect(await faucet.symbol()).to.equal("eQNTO");
    expect(await faucet.decimals()).to.equal(18);
  });

  // ── Claim (rate-limit) ─────────────────────────────────────────────────────

  it("allows first claim", async function () {
    await faucet.connect(alice).claim();
    expect(await faucet.balanceOf(alice.address)).to.equal(MINT_AMOUNT);
    expect(await faucet.totalSupply()).to.equal(MINT_AMOUNT);
  });

  it("emits Claimed and Transfer events on claim", async function () {
    await expect(faucet.connect(alice).claim())
      .to.emit(faucet, "Claimed")
      .withArgs(alice.address, MINT_AMOUNT, MINT_AMOUNT)
      .and.to.emit(faucet, "Transfer")
      .withArgs(ethers.ZeroAddress, alice.address, MINT_AMOUNT);
  });

  it("enforces per-address cooldown", async function () {
    await faucet.connect(alice).claim();
    await expect(faucet.connect(alice).claim())
      .to.be.revertedWithCustomError(faucet, "CooldownNotExpired");
  });

  it("allows second claim after cooldown", async function () {
    await faucet.connect(alice).claim();
    await time.increase(CLAIM_INTERVAL + 1);
    await faucet.connect(alice).claim();
    expect(await faucet.balanceOf(alice.address)).to.equal(MINT_AMOUNT * 2n);
  });

  it("allows independent claims from different addresses", async function () {
    await faucet.connect(alice).claim();
    await faucet.connect(bob).claim();
    expect(await faucet.balanceOf(alice.address)).to.equal(MINT_AMOUNT);
    expect(await faucet.balanceOf(bob.address)).to.equal(MINT_AMOUNT);
    expect(await faucet.totalSupply()).to.equal(MINT_AMOUNT * 2n);
  });

  it("claimFor reverts on zero address", async function () {
    await expect(faucet.connect(alice).claimFor(ethers.ZeroAddress))
      .to.be.revertedWithCustomError(faucet, "ZeroAddress");
  });

  it("claimFor works for a third party", async function () {
    await faucet.connect(alice).claimFor(bob.address);
    expect(await faucet.balanceOf(bob.address)).to.equal(MINT_AMOUNT);
    expect(await faucet.balanceOf(alice.address)).to.equal(0n);
  });

  // ── Supply cap ─────────────────────────────────────────────────────────────

  it("ownerMint enforces MAX_SUPPLY cap", async function () {
    // Mint right up to cap
    await faucet.connect(owner).ownerMint(alice.address, MAX_SUPPLY);
    // Another mint should revert
    await expect(faucet.connect(owner).ownerMint(alice.address, 1n))
      .to.be.revertedWithCustomError(faucet, "SupplyCapExceeded");
  });

  it("ownerMint reverts for non-owner", async function () {
    await expect(faucet.connect(alice).ownerMint(alice.address, MINT_AMOUNT))
      .to.be.revertedWithCustomError(faucet, "OwnableUnauthorizedAccount");
  });

  it("ownerMint reverts on zero address", async function () {
    await expect(faucet.connect(owner).ownerMint(ethers.ZeroAddress, MINT_AMOUNT))
      .to.be.revertedWithCustomError(faucet, "ZeroAddress");
  });

  // ── Pause circuit-breaker ──────────────────────────────────────────────────

  it("paused faucet blocks claim", async function () {
    await faucet.connect(owner).pause();
    await expect(faucet.connect(alice).claim())
      .to.be.revertedWithCustomError(faucet, "EnforcedPause");
  });

  it("unpausing restores claim", async function () {
    await faucet.connect(owner).pause();
    await faucet.connect(owner).unpause();
    await faucet.connect(alice).claim();
    expect(await faucet.balanceOf(alice.address)).to.equal(MINT_AMOUNT);
  });

  it("only owner can pause", async function () {
    await expect(faucet.connect(alice).pause())
      .to.be.revertedWithCustomError(faucet, "OwnableUnauthorizedAccount");
  });

  // ── View helpers ───────────────────────────────────────────────────────────

  it("remainingSupply decreases after mint", async function () {
    await faucet.connect(alice).claim();
    const remaining = await faucet.remainingSupply();
    expect(remaining).to.equal(MAX_SUPPLY - MINT_AMOUNT);
  });

  it("nextClaimTime returns correct timestamp", async function () {
    const tx = await faucet.connect(alice).claim();
    const block = await ethers.provider.getBlock(tx.blockNumber);
    const expected = BigInt(block.timestamp) + BigInt(CLAIM_INTERVAL);
    expect(await faucet.nextClaimTime(alice.address)).to.equal(expected);
  });
});

// ─── QUSD Tests ───────────────────────────────────────────────────────────────

describe("QUSD", function () {
  let qusd, admin, minter, pauser, alice, bob;

  const MAX_SUPPLY = ethers.parseEther("1000000000");
  const MINT_AMOUNT = ethers.parseEther("1000");

  const MINTER_ROLE = ethers.keccak256(ethers.toUtf8Bytes("MINTER_ROLE"));
  const PAUSER_ROLE = ethers.keccak256(ethers.toUtf8Bytes("PAUSER_ROLE"));
  const BURNER_ROLE = ethers.keccak256(ethers.toUtf8Bytes("BURNER_ROLE"));

  beforeEach(async function () {
    [admin, minter, pauser, alice, bob] = await ethers.getSigners();
    const QUSD = await ethers.getContractFactory("QUSD");
    qusd = await QUSD.deploy(admin.address, minter.address);
    await qusd.waitForDeployment();
  });

  // ── Deployment ─────────────────────────────────────────────────────────────

  it("starts with zero total supply", async function () {
    expect(await qusd.totalSupply()).to.equal(0n);
  });

  it("admin has DEFAULT_ADMIN_ROLE", async function () {
    const DEFAULT_ADMIN = await qusd.DEFAULT_ADMIN_ROLE();
    expect(await qusd.hasRole(DEFAULT_ADMIN, admin.address)).to.be.true;
  });

  it("minter has MINTER_ROLE", async function () {
    expect(await qusd.hasRole(MINTER_ROLE, minter.address)).to.be.true;
  });

  it("admin has PAUSER_ROLE", async function () {
    expect(await qusd.hasRole(PAUSER_ROLE, admin.address)).to.be.true;
  });

  it("has correct token metadata", async function () {
    expect(await qusd.name()).to.equal("Qanto USD");
    expect(await qusd.symbol()).to.equal("QUSD");
    expect(await qusd.decimals()).to.equal(18);
  });

  // ── Minting (role-gated) ───────────────────────────────────────────────────

  it("minter can mint tokens", async function () {
    await qusd.connect(minter).mint(alice.address, MINT_AMOUNT);
    expect(await qusd.balanceOf(alice.address)).to.equal(MINT_AMOUNT);
    expect(await qusd.totalSupply()).to.equal(MINT_AMOUNT);
  });

  it("non-minter cannot mint", async function () {
    await expect(qusd.connect(alice).mint(alice.address, MINT_AMOUNT))
      .to.be.revertedWithCustomError(qusd, "AccessControlUnauthorizedAccount");
  });

  it("mint reverts on zero address", async function () {
    await expect(qusd.connect(minter).mint(ethers.ZeroAddress, MINT_AMOUNT))
      .to.be.revertedWithCustomError(qusd, "ZeroAddress");
  });

  it("mint reverts on zero amount", async function () {
    await expect(qusd.connect(minter).mint(alice.address, 0n))
      .to.be.revertedWithCustomError(qusd, "ZeroAmount");
  });

  // ── Supply cap ─────────────────────────────────────────────────────────────

  it("mint enforces MAX_SUPPLY cap", async function () {
    await qusd.connect(minter).mint(alice.address, MAX_SUPPLY);
    await expect(qusd.connect(minter).mint(alice.address, 1n))
      .to.be.revertedWithCustomError(qusd, "SupplyCapExceeded");
  });

  it("remainingMintable decreases after mint", async function () {
    await qusd.connect(minter).mint(alice.address, MINT_AMOUNT);
    expect(await qusd.remainingMintable()).to.equal(MAX_SUPPLY - MINT_AMOUNT);
  });

  // ── Pause circuit-breaker ──────────────────────────────────────────────────

  it("paused contract blocks minting", async function () {
    await qusd.connect(admin).pause();
    await expect(qusd.connect(minter).mint(alice.address, MINT_AMOUNT))
      .to.be.revertedWithCustomError(qusd, "EnforcedPause");
  });

  it("paused contract blocks transfers", async function () {
    await qusd.connect(minter).mint(alice.address, MINT_AMOUNT);
    await qusd.connect(admin).pause();
    await expect(qusd.connect(alice).transfer(bob.address, MINT_AMOUNT))
      .to.be.revertedWithCustomError(qusd, "EnforcedPause");
  });

  it("only pauser can pause", async function () {
    await expect(qusd.connect(alice).pause())
      .to.be.revertedWithCustomError(qusd, "AccessControlUnauthorizedAccount");
  });

  it("unpausing restores minting", async function () {
    await qusd.connect(admin).pause();
    await qusd.connect(admin).unpause();
    await qusd.connect(minter).mint(alice.address, MINT_AMOUNT);
    expect(await qusd.balanceOf(alice.address)).to.equal(MINT_AMOUNT);
  });

  // ── Burn ───────────────────────────────────────────────────────────────────

  it("holder can burn own tokens", async function () {
    await qusd.connect(minter).mint(alice.address, MINT_AMOUNT);
    await qusd.connect(alice).burn(MINT_AMOUNT);
    expect(await qusd.balanceOf(alice.address)).to.equal(0n);
    expect(await qusd.totalSupply()).to.equal(0n);
  });

  it("BURNER_ROLE can burn from another account without allowance", async function () {
    await qusd.connect(minter).mint(alice.address, MINT_AMOUNT);
    await qusd.connect(admin).burnFrom(alice.address, MINT_AMOUNT);
    expect(await qusd.balanceOf(alice.address)).to.equal(0n);
  });

  it("non-burner cannot burn from another account without allowance", async function () {
    await qusd.connect(minter).mint(alice.address, MINT_AMOUNT);
    await expect(qusd.connect(bob).burnFrom(alice.address, MINT_AMOUNT))
      .to.be.revert(ethers);
  });

  // ── Role management ────────────────────────────────────────────────────────

  it("admin can grant MINTER_ROLE to a new account", async function () {
    await qusd.connect(admin).grantRole(MINTER_ROLE, alice.address);
    await qusd.connect(alice).mint(bob.address, MINT_AMOUNT);
    expect(await qusd.balanceOf(bob.address)).to.equal(MINT_AMOUNT);
  });

  it("admin can revoke MINTER_ROLE", async function () {
    await qusd.connect(admin).revokeRole(MINTER_ROLE, minter.address);
    await expect(qusd.connect(minter).mint(alice.address, MINT_AMOUNT))
      .to.be.revertedWithCustomError(qusd, "AccessControlUnauthorizedAccount");
  });
});
