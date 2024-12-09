import { Program, web3 } from '@project-serum/anchor';
import * as anchor from '@project-serum/anchor';
import fs from 'fs';
import NodeWallet from '@project-serum/anchor/dist/cjs/nodewallet';
import { PUMPSCIENCE, SEEDS, INIT_DEFAULTS, MIGRATION_VAULT } from './constants';
import { Connection, ComputeBudgetProgram, Transaction, PublicKey, SystemProgram, TransactionInstruction, TransactionMessage, VersionedTransaction, LAMPORTS_PER_SOL, SYSVAR_CLOCK_PUBKEY, AddressLookupTableProgram, Keypair as Web3JsKeypair } from '@solana/web3.js';
import VaultImpl, { getVaultPdas } from '@mercurial-finance/vault-sdk';
import { NATIVE_MINT } from '@solana/spl-token';
import { BN } from "bn.js";
import {
    getAssociatedTokenAccount
} from './util';
import AmmImpl, { PROGRAM_ID } from '@mercurial-finance/dynamic-amm-sdk';
import { IDL } from '../target/types/pump_science';
import { derivePoolAddressWithConfig, createProgram, getOrCreateATAInstruction, deriveMintMetadata, wrapSOLInstruction, deriveLockEscrowPda, getSolPriceInUSD } from './util'

let solConnection: web3.Connection = null;
let program: Program = null;
let provider: anchor.Provider = null;
let contractProvider: anchor.Provider = null;
let payer: NodeWallet = null;
const simpleMintKp = Web3JsKeypair.generate();
const connection = new Connection("rpc");

// Address of the deployed program.
let programId = new anchor.web3.PublicKey(PUMPSCIENCE);
/**
 * Set cluster, provider, program
 * If rpc != null use rpc, otherwise use cluster param
 * @param cluster - cluster ex. mainnet-beta, devnet ...
 * @param keypair - wallet keypair
 * @param rpc - rpc
 */
export const setClusterConfig = async (
    cluster: web3.Cluster,
    keypair: string, rpc?: string
) => {

    if (!rpc) {
        solConnection = new web3.Connection(web3.clusterApiUrl(cluster));
    } else {
        solConnection = new web3.Connection(rpc);
    }

    const walletKeypair = web3.Keypair.fromSecretKey(
        Uint8Array.from(JSON.parse(fs.readFileSync(keypair, 'utf-8'))),
        { skipValidation: true });

    const wallet = new NodeWallet(walletKeypair);

    // Configure the client to use the local cluster.
    anchor.setProvider(new anchor.AnchorProvider(
        solConnection,
        wallet,
        { skipPreflight: true, commitment: 'confirmed' }));
    payer = wallet;

    provider = anchor.getProvider();
    contractProvider = new anchor.AnchorProvider(
        connection,
        wallet,
        { skipPreflight: true, commitment: 'confirmed' }
    )

    // Generate the program client from IDL.
    program = new anchor.Program(IDL as anchor.Idl, programId);
}

export const global = async () => {

    const global = PublicKey.findProgramAddressSync([Buffer.from("global")], PUMPSCIENCE)[0];
    const eventAuthority = PublicKey.findProgramAddressSync([Buffer.from("__event_authority")], PUMPSCIENCE)[0];
    const migrateVault = new PublicKey("3bM4hewuZFZgNXvLWwaktXMa8YHgxsnnhaRfzxJV944P")
    const solPrice = await getSolPriceInUSD();
    console.log("global", global.toBase58());

    INIT_DEFAULTS.migrateFeeAmount = new BN(Number(INIT_DEFAULTS.migrateFeeAmount) / solPrice * LAMPORTS_PER_SOL);

    const tx = await program.methods.initialize(INIT_DEFAULTS).accounts({
        global,
        eventAuthority,
        systemProgram: SystemProgram.programId,
        program: programId
    }).transaction();
    console.log("here1");

    const latestBlockHash = await provider.connection.getLatestBlockhash(
        provider.connection.commitment,
    );
    const creatTx = new web3.Transaction({
        feePayer: payer.publicKey,
        ...latestBlockHash,
    }).add(tx)

    creatTx.sign(payer.payer);

    const preInxSim = await solConnection.simulateTransaction(creatTx)

    const txHash = await provider.sendAndConfirm(creatTx, [], {
        commitment: "finalized",
    });

    return txHash;
}

export const migrate = async () => {
    const { ammProgram, vaultProgram } = createProgram(provider.connection, null);
    const eventAuthority = PublicKey.findProgramAddressSync([Buffer.from("__event_authority")], new PublicKey(PROGRAM_ID))[0];

    const global = PublicKey.findProgramAddressSync([Buffer.from("global")], PUMPSCIENCE)[0];

    console.log("global--->>>", global.toBase58());
    const tokenAMint = NATIVE_MINT;
    const tokenBMint = new PublicKey('6z6XvMUvCrKxaUGyBxKC4sCRoikcuG4TNZ7afeBATGsA');
    const config = new PublicKey('21PjsfQVgrn56jSypUT5qXwwSjwKWvuoBCKbVZrgTLz4');
    const bondingCurve = PublicKey.findProgramAddressSync([Buffer.from("bonding-curve"), tokenBMint.toBuffer()], PUMPSCIENCE)[0];
    // const feeVault = PublicKey.findProgramAddressSync([Buffer.from("fee-vault"), tokenBMint.toBuffer()], PUMPSCIENCE)[0];
    let tokenAAmount = new BN(0.01 * 1000000000);
    let tokenBAmount = new BN(10 * 1000000000);

    const poolPubkey = derivePoolAddressWithConfig(tokenAMint, tokenBMint, config, ammProgram.programId);

    const [
        { vaultPda: aVault, tokenVaultPda: aTokenVault, lpMintPda: aLpMintPda },
        { vaultPda: bVault, tokenVaultPda: bTokenVault, lpMintPda: bLpMintPda },
    ] = [getVaultPdas(tokenAMint, vaultProgram.programId), getVaultPdas(tokenBMint, vaultProgram.programId)];

    let aVaultLpMint = aLpMintPda;
    let bVaultLpMint = bLpMintPda;
    let preInstructions: Array<TransactionInstruction> = [];

    const [aVaultAccount, bVaultAccount] = await Promise.all([
        vaultProgram.account.vault.fetchNullable(aVault),
        vaultProgram.account.vault.fetchNullable(bVault),
    ]);

    if (!aVaultAccount) {
        const createVaultAIx = await VaultImpl.createPermissionlessVaultInstruction(provider.connection, payer.publicKey, tokenAMint);
        createVaultAIx && preInstructions.push(createVaultAIx);

    } else {
        aVaultLpMint = aVaultAccount.lpMint; // Old vault doesn't have lp mint pda
    }
    if (!bVaultAccount) {
        const createVaultBIx = await VaultImpl.createPermissionlessVaultInstruction(provider.connection, payer.publicKey, tokenBMint);
        createVaultBIx && preInstructions.push(createVaultBIx);

    } else {
        bVaultLpMint = bVaultAccount.lpMint; // Old vault doesn't have lp mint pda
    }

    const [lpMint] = PublicKey.findProgramAddressSync(
        [Buffer.from(SEEDS.LP_MINT), poolPubkey.toBuffer()],
        ammProgram.programId,
    );
    const [[aVaultLp], [bVaultLp]] = [
        PublicKey.findProgramAddressSync([aVault.toBuffer(), poolPubkey.toBuffer()], ammProgram.programId),
        PublicKey.findProgramAddressSync([bVault.toBuffer(), poolPubkey.toBuffer()], ammProgram.programId),
    ];

    const [[payerTokenA, createPayerTokenAIx], [payerTokenB, createPayerTokenBIx]] = await Promise.all([
        getOrCreateATAInstruction(tokenAMint, payer.publicKey, provider.connection),
        getOrCreateATAInstruction(tokenBMint, payer.publicKey, provider.connection),
    ]);

    createPayerTokenAIx && preInstructions.push(createPayerTokenAIx);
    createPayerTokenBIx && preInstructions.push(createPayerTokenBIx);

    const [[protocolTokenAFee], [protocolTokenBFee]] = [
        PublicKey.findProgramAddressSync(
            [Buffer.from(SEEDS.FEE), tokenAMint.toBuffer(), poolPubkey.toBuffer()],
            ammProgram.programId,
        ),
        PublicKey.findProgramAddressSync(
            [Buffer.from(SEEDS.FEE), tokenBMint.toBuffer(), poolPubkey.toBuffer()],
            ammProgram.programId,
        ),
    ];

    const payerPoolLp = getAssociatedTokenAccount(lpMint, payer.publicKey);

    if (tokenAMint.equals(NATIVE_MINT)) {
        preInstructions = preInstructions.concat(wrapSOLInstruction(payer.publicKey, payerTokenA, BigInt(tokenAAmount.toString())));
    }

    if (tokenBMint.equals(NATIVE_MINT)) {
        preInstructions = preInstructions.concat(
            wrapSOLInstruction(
                payer.publicKey,
                payerTokenB,
                BigInt(tokenBAmount.add(new BN(0)).toString()),
            ),
        );
    }
    const setComputeUnitLimitIx = ComputeBudgetProgram.setComputeUnitLimit({
        units: 20_000_000,
    });
    let latestBlockHash = await ammProgram.provider.connection.getLatestBlockhash(
        ammProgram.provider.connection.commitment,
    );

    if (preInstructions.length) {
        const preInstructionTx = new Transaction({
            feePayer: payer.publicKey,
            ...latestBlockHash,
        }).add(...preInstructions);

        preInstructionTx.sign(payer.payer);
        const preInxSim = await solConnection.simulateTransaction(preInstructionTx)

        const txHash = await provider.sendAndConfirm(preInstructionTx, [], {
            commitment: "finalized",
        });
    }

    const [mintMetadata, _mintMetadataBump] = deriveMintMetadata(lpMint);
    const [lockEscrowPK] = deriveLockEscrowPda(poolPubkey, payer.publicKey, ammProgram.programId);
    const [escrowAta, createEscrowAtaIx] = await getOrCreateATAInstruction(lpMint, lockEscrowPK, connection, payer.publicKey);
    console.log("bonding curve:", bondingCurve.toBase58());
    const migrationVault = MIGRATION_VAULT;
    const txLockPool = await program.methods
        .lockPool(tokenAAmount, tokenBAmount)
        .accounts({
           // lock accounts
        })
        .transaction();
    console.log("========== lock ==============");
    
    const txCreatePool = await program.methods
        .createLockPool(tokenAAmount, tokenBAmount)
        .accounts({
            // meteora create accounts
        })
        .transaction();

    const creatTx = new web3.Transaction({
        feePayer: payer.publicKey,
        ...latestBlockHash,
    }).add(setComputeUnitLimitIx).add(txCreatePool)

    const [lookupTableInst, lookupTableAddress] =
        AddressLookupTableProgram.createLookupTable({
            authority: payer.publicKey,
            payer: payer.publicKey,
            recentSlot: await contractProvider.connection.getSlot(),
        });

    const addresses = [
        // meteora create accounts
    ]

    const addAddressesInstruction1 = AddressLookupTableProgram.extendLookupTable({
        payer: payer.publicKey,
        authority: payer.publicKey,
        lookupTable: lookupTableAddress,
        addresses: addresses.slice(0, 30)
    });

    latestBlockHash = await ammProgram.provider.connection.getLatestBlockhash(
        ammProgram.provider.connection.commitment,
    );

    const lutMsg1 = new TransactionMessage({
        payerKey: payer.publicKey,
        recentBlockhash: latestBlockHash.blockhash,
        instructions: [lookupTableInst, addAddressesInstruction1]
    }).compileToV0Message();

    const lutVTx1 = new VersionedTransaction(lutMsg1);
    lutVTx1.sign([payer.payer])

    const lutId1 = await contractProvider.connection.sendTransaction(lutVTx1)
    const lutConfirm1 = await contractProvider.connection.confirmTransaction(lutId1, 'finalized')

    const lookupTableAccount = await contractProvider.connection.getAddressLookupTable(lookupTableAddress, { commitment: 'finalized' })

    const createTxMsg = new TransactionMessage({
        payerKey: payer.publicKey,
        recentBlockhash: latestBlockHash.blockhash,
        instructions: creatTx.instructions
        // }).compileToV0Message();
    }).compileToV0Message([lookupTableAccount.value]);

    const createVTx = new VersionedTransaction(createTxMsg);
    createVTx.sign([payer.payer])

    const sim = await contractProvider.connection.simulateTransaction(createVTx, { sigVerify: true })

    const id = await contractProvider.connection.sendTransaction(createVTx)
    const confirm = await contractProvider.connection.confirmTransaction(id)

    const lockPoolTxMsg = new TransactionMessage({
        payerKey: payer.publicKey,
        recentBlockhash: latestBlockHash.blockhash,
        instructions: txLockPool.instructions
        // }).compileToV0Message();
    }).compileToV0Message([lookupTableAccount.value]);

    const lockPoolVTx = new VersionedTransaction(lockPoolTxMsg);
    lockPoolVTx.sign([payer.payer])

    const lockPoolSim = await contractProvider.connection.simulateTransaction(lockPoolVTx, { sigVerify: true })

    const lockPoolId = await contractProvider.connection.sendTransaction(lockPoolVTx)
    const lockPoolConfirm = await contractProvider.connection.confirmTransaction(lockPoolId)

    return lockPoolId;
}
