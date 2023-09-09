# Include Shillables Contracts to the list in v1.10 for Pre v1.10 Contracts

## Summary

Include Shillables contracts in the list of pre v1.10 contracts in the v1.10 upgrade\n\n## Summary\n\nThis is a signaling proposal by Shillables to append their contract to the list of pre v1.10 contracts in the v1.10 upgrade. We missed the opportunity to be included in prop #262 and are asking kindly to be appended to the list. This will allow these contracts to be upgraded to the new version without having to create a new contract and have users manually migrate their state.

If approved, the v1.10 upgrade, tentatively scheduled for the 12th of September, 2023, will include the proposed hardcoded admins in its code.

## Details

If this proposal passes, the list of contracts that will be hardcoded into the v1.10 upgrade is as follows:

| Contract                                      | Description                   | TVL | Current code ID | Current source code | Reason for wanting to upgrade | New admin address                             | Admin account type |
| --------------------------------------------- | ----------------------------- | --- | --------------- | ------------------- | ----------------------------- | --------------------------------------------- | ------------------ |
| secret1f4r3jc07jk08xm8thdgmzd3y470e0k3d3k7r6p | ShillStake ($SHILL)           |     | 1004            |                     | Add multiple rewards          | secret1ght5566c9w3kdck90ywx9ky247nay98cc0qt0g | Shillables Team    |
| secret19agqymmc54jwcnhu06wzcwpkjkr86hdf0eydru | ShillStake (Sly Foxes)        |     | 1027            |                     | Add multiple rewards          | secret1ght5566c9w3kdck90ywx9ky247nay98cc0qt0g | Shillables Team    |
| secret14h8c2nwfh4et0t8tagse7faz3s3hqe9ty7evfk | ShillStake (Ample Agents LLC) |     | 1027            |                     | Add multiple rewards          | secret1ght5566c9w3kdck90ywx9ky247nay98cc0qt0g | Shillables Team    |
| secret1gcfn4ycc4afqapkvxd8ws6l7ahjcw77849awfg | ShillStake (Catyclops)        |     | 1027            |                     | Add multiple rewards          | secret1ght5566c9w3kdck90ywx9ky247nay98cc0qt0g | Shillables Team    |
| secret1nvmymjpu359sm2fpjl3hpcchfve0y88lz9jfye | ShillStake (BananAppeals)     |     | 1027            |                     | Add multiple rewards          | secret1ght5566c9w3kdck90ywx9ky247nay98cc0qt0g | Shillables Team    |
| secret1klssqs6ws59frztrnxndnvksh6v9ftyd0hyud9 | ShillStake (Boonanas)         |     | 1027            |                     | Add multiple rewards          | secret1ght5566c9w3kdck90ywx9ky247nay98cc0qt0g | Shillables Team    |
| secret1xlzwfuqwpasppsmtuna2k3mak69cwkc0pkyl6r | ShillStake (Alphacas)         |     | 1027            |                     | Add multiple rewards          | secret1ght5566c9w3kdck90ywx9ky247nay98cc0qt0g | Shillables Team    |
| secret10u4stpj7qpl3va2s94e03legaeqczdlhjvgc3f | ShillStake (Wolf Pack Alphas) |     | 1048            |                     | Add multiple rewards          | secret1ght5566c9w3kdck90ywx9ky247nay98cc0qt0g | Shillables Team    |
| secret197dvnt9yjxwn8sjdlx05f7zuk27lsdxtfnwxse | SHILL SNIP-25                 |     | 958             |                     | Add MetaMask permits & decoys | secret1ght5566c9w3kdck90ywx9ky247nay98cc0qt0g | Shillables Team    |
| secret16majzwc2q9sgy7ufcfmn5vnmes88l34nj78f7m | Wolf Pack PackBuilder         |     | 1019            |                     | Add/Remove payment methods    | secret1ght5566c9w3kdck90ywx9ky247nay98cc0qt0g | Shillables Team    |

## Key Takeaways

This proposal will allow the listed contracts to be upgraded to the new version without having to create a new contract or manually migrating user data. This will save time and effort for developers and make it easier for users to continue using the contracts after the v1.10 upgrade.

The Shillables team is seeking to upgrade all of their Shill Stake contracts to allow for multiple rewards to be given without impacting the users. The Wolf Pack PackBuilder is included to be able to fix a problem with the contract where new payment methods can not be added. SHILL is included in this list in case there are any key privacy updates.

## Risks

The following is copied from proposal #262.

The main risk of this proposal is that hardcoded admins could be used to upgrade contracts to malicious code that could leak private data or steal funds. To mitigate this risk, hardcoded admins should be carefully chosen and the chain should be monitored for suspicious `MsgMigrateContract` transactions.

Note: Hardcoded admins can only be changed or removed by a governance proposal and a subsequent chain upgrade.

For more info: [https://forum.scrt.network/t/an-update-on-the-contract-upgrade-feature/7012](https://forum.scrt.network/t/an-update-on-the-contract-upgrade-feature/7012)
