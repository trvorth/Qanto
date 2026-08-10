// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/utils/cryptography/MerkleProof.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

contract QantoDrop is Ownable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    IERC20 public immutable qntoToken;
    bytes32 public merkleRoot;
    mapping(bytes32 => mapping(address => bool)) private _claimedByRoot;

    event Claimed(address indexed claimant, uint256 amount);
    event MerkleRootUpdated(bytes32 newRoot);

    constructor(address _token, bytes32 _merkleRoot) Ownable(msg.sender) {
        require(_token != address(0), "QantoDrop: Zero token.");
        require(_merkleRoot != bytes32(0), "QantoDrop: Empty root.");
        qntoToken = IERC20(_token);
        merkleRoot = _merkleRoot;
    }

    function updateMerkleRoot(bytes32 _merkleRoot) external onlyOwner {
        require(_merkleRoot != bytes32(0), "QantoDrop: Empty root.");
        require(_merkleRoot != merkleRoot, "QantoDrop: Root unchanged.");
        merkleRoot = _merkleRoot;
        emit MerkleRootUpdated(_merkleRoot);
    }

    function hasClaimed(address account) external view returns (bool) {
        return _claimedByRoot[merkleRoot][account];
    }

    function claim(uint256 amount, bytes32[] calldata merkleProof) external nonReentrant {
        require(!_claimedByRoot[merkleRoot][msg.sender], "QantoDrop: Airdrop already claimed.");
        
        // Verify the merkle proof
        bytes32 node = keccak256(abi.encodePacked(msg.sender, amount));
        require(MerkleProof.verify(merkleProof, merkleRoot, node), "QantoDrop: Invalid proof.");

        _claimedByRoot[merkleRoot][msg.sender] = true;
        qntoToken.safeTransfer(msg.sender, amount);
        
        emit Claimed(msg.sender, amount);
    }

    function withdrawRemaining() external onlyOwner {
        uint256 balance = qntoToken.balanceOf(address(this));
        qntoToken.safeTransfer(owner(), balance);
    }
}
