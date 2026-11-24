# Changelog

All notable changes to async-tar will be documented in this file.

<!-- git-cliff: end of header -->
## [0.7.0](https://github.com/dignifiedquire/async-tar/compare/v0.6.2..v0.7.0) - 2026-11-24

### ⛰️  Features

- [**breaking**] Replace async-std with smol ([#91](https://github.com/dignifiedquire/async-tar/issues/91)) - ([01a7bb3](https://github.com/dignifiedquire/async-tar/commit/01a7bb3827035868f052076e554018120f94d05d))
- Make the extension header and sparse map limits configurable - ([b51a3dd](https://github.com/dignifiedquire/async-tar/commit/b51a3dd3ad8a14700e1bb4c3304e222935e70910))

### 🐛 Bug Fixes

- [**breaking**] Drop the Send and Sync bounds on Builder ([#87](https://github.com/dignifiedquire/async-tar/issues/87)) - ([ef50a03](https://github.com/dignifiedquire/async-tar/commit/ef50a03db783045b3bacf05cc8e187f711729bce))
- [**breaking**] Reject ambiguous tar archives ([#94](https://github.com/dignifiedquire/async-tar/issues/94)) - ([fd3ab9b](https://github.com/dignifiedquire/async-tar/commit/fd3ab9bdbbb7d7cc369d053c2737db4b00e11a8b))
- End pax records at a NUL - ([299603f](https://github.com/dignifiedquire/async-tar/commit/299603f595a4c55fe873de849925d42c4a5538fd))
- Reject member sizes beyond i64::MAX - ([60010e7](https://github.com/dignifiedquire/async-tar/commit/60010e7554b63d6f616519f00cecbe90a2974129))
- Reject a name ending in a slash on a member with data - ([40b7c64](https://github.com/dignifiedquire/async-tar/commit/40b7c6400af7a6d3c310fdb5dd97fd9c7f0b6d46))
- Cap GNU long names and pax headers at 1 MiB - ([7a45bf4](https://github.com/dignifiedquire/async-tar/commit/7a45bf4bec82d1dc29f677eb9e35f1025be96c84))
- Cap old GNU sparse maps at 1 MiB - ([8049318](https://github.com/dignifiedquire/async-tar/commit/8049318e30ca053f46dfcab4afea0792b2cf0761))
- Read sparse entries in linear time - ([cd7b352](https://github.com/dignifiedquire/async-tar/commit/cd7b3522e5c3b8d456a19574428ef5dc586b24a8))
- Stop creating directories at the first error but NotFound - ([f6a371c](https://github.com/dignifiedquire/async-tar/commit/f6a371c698e98515cf3e6234c338a7e119e3e564))
- Reject a NUL inside a name or link target - ([402e751](https://github.com/dignifiedquire/async-tar/commit/402e7513560a15c63974be438c03fb3bbd547307))
- Fail reads of a member that ends before its size - ([635738b](https://github.com/dignifiedquire/async-tar/commit/635738bb7218135aea38852af1788d9bf290597e))
- Reject a GNU sparse map that continues after an empty slot - ([4bd5f12](https://github.com/dignifiedquire/async-tar/commit/4bd5f1254515106484dde376736441d86b17c677))
- Read any nonzero GNU sparse isextended flag as extended - ([415cc0e](https://github.com/dignifiedquire/async-tar/commit/415cc0e6d343393a5b72ceda1b13b3309d63ecc4))
- Validate a long path before writing its GNU long name - ([0583e3d](https://github.com/dignifiedquire/async-tar/commit/0583e3d0e062aa3c362bcd3660e223d2e414ee13))
- [**breaking**] Clear group and other write on unpack by default - ([a6529c5](https://github.com/dignifiedquire/async-tar/commit/a6529c5aae467ed786d7a02464db44caa7f0a115))
- Write a member's extension headers only after all its fields are set - ([39e9a11](https://github.com/dignifiedquire/async-tar/commit/39e9a11ea02f38b9869c38569559e9edb507a253))
- Reject a colon in a name component on Windows - ([7dbd050](https://github.com/dignifiedquire/async-tar/commit/7dbd05015989ee69e3dc1a27d321f0716749d664))
- Refuse to write after a member was left incomplete - ([aea0886](https://github.com/dignifiedquire/async-tar/commit/aea088656f53c661448207f15d0c9637658540d8))
- Write exactly a member's header size of data - ([0199378](https://github.com/dignifiedquire/async-tar/commit/019937886c783a36fe1dc38885bf8c33c1065ffb))
- Reject Windows device names when unpacking on Windows - ([44965fb](https://github.com/dignifiedquire/async-tar/commit/44965fbc45c5f15672e3b69eaad3defee93da295))

### 🚜 Refactor

- Expand lints - ([6938648](https://github.com/dignifiedquire/async-tar/commit/69386488abf5bad2d729c707aacb601afd2a0c8e))
- Build an empty GNU sparse extension header without unsafe code - ([142553c](https://github.com/dignifiedquire/async-tar/commit/142553c1a7db1670f94c178f30fd36eb414c9cf3))
- View headers through zerocopy's checked transmutes - ([2d3f460](https://github.com/dignifiedquire/async-tar/commit/2d3f460fa6808c1e75b5ea8866c0f0fab913dadf))
- Remove unsafe code from the header views ([#105](https://github.com/dignifiedquire/async-tar/issues/105)) - ([50ea38e](https://github.com/dignifiedquire/async-tar/commit/50ea38ea1465476abf1fe83bbbded9a5a2a0027e))

### 📚 Documentation

- Describe what Builder::finish writes and that it does not flush ([#88](https://github.com/dignifiedquire/async-tar/issues/88)) - ([5b9d746](https://github.com/dignifiedquire/async-tar/commit/5b9d746b9b3188c34da3faa9611a35892d29ceb8))
- Recommend a buffered reader for tokio files - ([373af68](https://github.com/dignifiedquire/async-tar/commit/373af68b6dd4754285fda6ec83d402c35a15b961))
- Update reference reader comments - ([e361293](https://github.com/dignifiedquire/async-tar/commit/e3612938ca3a46ae54ee37254184ded0a9cbfc5e))

### ⚡ Performance

- Write octal header fields without allocating - ([016f80c](https://github.com/dignifiedquire/async-tar/commit/016f80c87c698fe65ff1e91b62309120dc35a79b))
- Skip unread data through an uninitialized buffer on tokio - ([56e49fd](https://github.com/dignifiedquire/async-tar/commit/56e49fd99df3bac176d103a4f01186a5a6f91a38))
- Canonicalize the unpack destination once - ([3f19c15](https://github.com/dignifiedquire/async-tar/commit/3f19c15f025ae0e8cfdef97ca2890f21f57e24b6))
- Copy file data in chunks of up to 128 KiB - ([56c0551](https://github.com/dignifiedquire/async-tar/commit/56c05515bd9cb5cfb3e36d6b56fc91297288e81c))
- Read directories and small files in batched blocking calls when building - ([23100f1](https://github.com/dignifiedquire/async-tar/commit/23100f141edc2d6dea352dc9704d6649dd072dc3))
- Create, write and timestamp small files in one blocking call - ([001ba6c](https://github.com/dignifiedquire/async-tar/commit/001ba6ce3ede27de1d8daa472c682a422e8eff29))
- Unpack each small file in one blocking call - ([8d8820e](https://github.com/dignifiedquire/async-tar/commit/8d8820e15a94dcf8d9f0eddaa15de0a9c7d64873))
- Write consecutive small files in batched blocking calls - ([12f5b30](https://github.com/dignifiedquire/async-tar/commit/12f5b303f3acfdf0eabab6e080547fff9b932aaa))

### ⚙️ Miscellaneous Tasks

- Run on pushes to main ([#92](https://github.com/dignifiedquire/async-tar/issues/92)) - ([5327304](https://github.com/dignifiedquire/async-tar/commit/5327304eae86ddb1638d3d8f7a6dcf9be6e9e4ef))
- Check the default features on aarch64-linux-android ([#89](https://github.com/dignifiedquire/async-tar/issues/89)) - ([8a6a834](https://github.com/dignifiedquire/async-tar/commit/8a6a8341bfbba7fcb5d38bf876fcdd30dd17a2a3))
- Generate the changelog with git-cliff ([#93](https://github.com/dignifiedquire/async-tar/issues/93)) - ([cb59698](https://github.com/dignifiedquire/async-tar/commit/cb59698826721be695e04eebc8ad77c4f72c52c9))
- Ship the test archives in the published crate ([#90](https://github.com/dignifiedquire/async-tar/issues/90)) - ([443c3c6](https://github.com/dignifiedquire/async-tar/commit/443c3c63af5babea3e0a682c4b8222bf71015c50))
- Fix all warnings and deny them in CI ([#95](https://github.com/dignifiedquire/async-tar/issues/95)) - ([d474c5d](https://github.com/dignifiedquire/async-tar/commit/d474c5d2866b2ea03bc0ef3090140ff40f160b6c))
- Run formatting and clippy through cargo make ([#96](https://github.com/dignifiedquire/async-tar/issues/96)) - ([266c9a4](https://github.com/dignifiedquire/async-tar/commit/266c9a4dc264e60a241de8bf2fa9a2a4c7987728))
- Pin actions and check workflows and dependencies ([#97](https://github.com/dignifiedquire/async-tar/issues/97)) - ([a2b6824](https://github.com/dignifiedquire/async-tar/commit/a2b682422469ce3ddce9d4e71590d5a8743e20a9))


## [0.6.2](https://github.com/dignifiedquire/async-tar/compare/v0.6.1..v0.6.2) - 2026-11-23

### 🐛 Bug Fixes

- *(pax)* Parse PAX extensions by record length instead of splitting on newlines ([#73](https://github.com/dignifiedquire/async-tar/issues/73)) - ([695a4e5](https://github.com/dignifiedquire/async-tar/commit/695a4e54c0482873e7b8ab40b8cb1d1cfb81074c))
- Use Vec::clear for sparse block reset - ([f07d4bf](https://github.com/dignifiedquire/async-tar/commit/f07d4bfa08f30828f4ff4117fe35174b953e1319))
- Allow the deprecated filetime::set_file_times - ([f87e631](https://github.com/dignifiedquire/async-tar/commit/f87e631a0d3923077dd28f08211f1bb7272068b9))
- Do not follow symlinks when unpacking over an existing directory - ([2d979f2](https://github.com/dignifiedquire/async-tar/commit/2d979f2e332e724acbaf6395a56726f777be3731))
- Use the last repeated PAX path and read PAX linkpath - ([25b4a01](https://github.com/dignifiedquire/async-tar/commit/25b4a0157da278fd86f7469f4b55e8d50fa1c024))
- Read the extension headers of every GNU sparse entry - ([ede367e](https://github.com/dignifiedquire/async-tar/commit/ede367e9b48f4f59422b22d93dca7dcb9c7b7484))
- Keep extension data across Pending reads - ([0725490](https://github.com/dignifiedquire/async-tar/commit/07254900e5e6a1897c84efb95048ae7107af1e72))
- End the entries stream after an error - ([32443bd](https://github.com/dignifiedquire/async-tar/commit/32443bd7d69a35c8aa67c48e797ca8fce5c46b76))
- Read ustar headers whose version is two NULs - ([43d2eeb](https://github.com/dignifiedquire/async-tar/commit/43d2eebbcf6538ab183f51bfbde0264a0d54c2f4))
- Reject a size on links, directories, devices and FIFOs - ([953f838](https://github.com/dignifiedquire/async-tar/commit/953f838bf6e15173c4892df698614cecd807a239))
- Reject a GNU long name and a PAX path on one member - ([3c5c9d9](https://github.com/dignifiedquire/async-tar/commit/3c5c9d96ea08b0844efec50b8e1bb3d5f14ccd20))
- Reject a PAX global header between an extension and its member - ([8e0c180](https://github.com/dignifiedquire/async-tar/commit/8e0c1807000afc0ed426dc48c16a4851feaa1e63))
- Reject extension headers async-tar does not read - ([27d1430](https://github.com/dignifiedquire/async-tar/commit/27d14303f4e25af68c5e29b261c67df95629544e))
- Reject GNU sparse PAX records - ([43b1ef1](https://github.com/dignifiedquire/async-tar/commit/43b1ef1c3e531d370761a677b7ee0248ff36af82))
- Reject an empty PAX path or linkpath - ([e13d452](https://github.com/dignifiedquire/async-tar/commit/e13d452497ea99798dc937086e55857e6637c1c8))
- Do not unpack GNU volume labels as files - ([ad33573](https://github.com/dignifiedquire/async-tar/commit/ad335739a2d6333d51585f1fc3ae85727b6e836d))
- Reject malformed PAX records - ([34a547d](https://github.com/dignifiedquire/async-tar/commit/34a547db128c9538064145c0dee0c8795a625ac6))
- Security fixes for 0.6.2 ([#86](https://github.com/dignifiedquire/async-tar/issues/86)) - ([a66a79a](https://github.com/dignifiedquire/async-tar/commit/a66a79aa11834c78d4264f010b66f4ae3023c4d0))

### ⚙️ Miscellaneous Tasks

- Bump actions/checkout from 4 to 7 - ([c5799f8](https://github.com/dignifiedquire/async-tar/commit/c5799f8e466445792652927d522b012b1b780fb2))
- Drop duplicate key in Cargo.toml - ([7d19316](https://github.com/dignifiedquire/async-tar/commit/7d19316e9f479079b4f19e9659323ee327ee3dcf))

## [0.6.1](https://github.com/dignifiedquire/async-tar/compare/v0.6.0..v0.6.1) - 2026-06-01

### 🐛 Bug Fixes

- Do not leak overrides into the next poll call - ([e2443a7](https://github.com/dignifiedquire/async-tar/commit/e2443a7e90757d831781e5bdc3bba096d3668fd8))

### 🧪 Testing

- Add failing test for GHSA-35rm-7j9c-2f7m (PAX extension-header desync) - ([9ee2991](https://github.com/dignifiedquire/async-tar/commit/9ee2991a06a04b970a37ae6ece44047cfcbd6c4a))

## [0.6.0](https://github.com/dignifiedquire/async-tar/compare/v0.5.1..v0.6.0) - 2026-01-23

### ⛰️  Features

- [**breaking**] Introduce tokio compatability - ([41e0587](https://github.com/dignifiedquire/async-tar/commit/41e058705f60041a4cedde1de6b5d249ad78d579))

## [0.5.1](https://github.com/dignifiedquire/async-tar/compare/v0.5.0..v0.5.1) - 2025-10-22

### 🐛 Bug Fixes

- Patch for tarbug - ([0c18195](https://github.com/dignifiedquire/async-tar/commit/0c181956395fa28a9ae45786a57c9cf58c413e84))

### ⚙️ Miscellaneous Tasks

- Bump actions/checkout from 2 to 4 ([#53](https://github.com/dignifiedquire/async-tar/issues/53)) - ([e1323ee](https://github.com/dignifiedquire/async-tar/commit/e1323ee8d47697f7988a9a104f860f73d3797d65))

## [0.5.0](https://github.com/dignifiedquire/async-tar/compare/v0.4.2..v0.5.0) - 2024-09-02

### ⛰️  Features

- MSRV 1.63 - ([611d903](https://github.com/dignifiedquire/async-tar/commit/611d903871ef5de0ea321e25fae9e08100140195))

### 🐛 Bug Fixes

- Preserve intermediate entry state - ([b5fb2d7](https://github.com/dignifiedquire/async-tar/commit/b5fb2d7b1f0073b39dd7cb3a38e6bc925950b1ca))
- Update async-std to use unstable flag - ([b4372dd](https://github.com/dignifiedquire/async-tar/commit/b4372ddb50a38c12d2da939f99addad807b06881))

### 🚜 Refactor

- Use 'mem::take' instead of 'mem::replace' - ([b6975b7](https://github.com/dignifiedquire/async-tar/commit/b6975b7095f71b461e1f09eb52a21b86e03dcffb))

### 🧪 Testing

- Fix windows test - ([c8b8ec1](https://github.com/dignifiedquire/async-tar/commit/c8b8ec14695cb7b64b5361b3097d28b8e2bd7218))

### ⚙️ Miscellaneous Tasks

- Add dependabot config - ([365c331](https://github.com/dignifiedquire/async-tar/commit/365c331af89bf5c9d34cc0f145bceb82391cef9f))
- Bump codecov/codecov-action from 1 to 2.1.0 - ([748fee5](https://github.com/dignifiedquire/async-tar/commit/748fee5c70ee3aae297ff5e85ab11371bbb52407))
- Apply clippy - ([1f4775a](https://github.com/dignifiedquire/async-tar/commit/1f4775a10b4377e24b0491782926bd8905390c5a))
- Remove broken coverage workflow - ([0535ab2](https://github.com/dignifiedquire/async-tar/commit/0535ab2439909be5399d79f955a108307f7f0eea))
- Run fmt and clippy on stable - ([5a9cdd4](https://github.com/dignifiedquire/async-tar/commit/5a9cdd49725829b68aba1836cabc69c025ab7198))

## [0.4.2](https://github.com/dignifiedquire/async-tar/compare/v0.4.1..v0.4.2) - 2021-08-24

### 🐛 Bug Fixes

- Do not create directories outside ([#24](https://github.com/dignifiedquire/async-tar/issues/24)) - ([9e61431](https://github.com/dignifiedquire/async-tar/commit/9e614315603e9a28dff1bd0a813cf2c96e517143))

## [0.4.1](https://github.com/dignifiedquire/async-tar/compare/v0.4.0..v0.4.1) - 2021-07-29

### 🐛 Bug Fixes

- Use resolver 2 and full async-std for now - ([7900b2c](https://github.com/dignifiedquire/async-tar/commit/7900b2c02ae26230046017f105041601a7573ed2))

## [0.4.0](https://github.com/dignifiedquire/async-tar/compare/v0.3.0..v0.4.0) - 2021-07-28

### ⛰️  Features

- *(deps)* Update pin-project and redox_syscall - ([468e323](https://github.com/dignifiedquire/async-tar/commit/468e32382c5a326c256bdd9af451a6dc2bb1a523))

### ⚙️ Miscellaneous Tasks

- Happy clippy - ([33b9b42](https://github.com/dignifiedquire/async-tar/commit/33b9b420e243e74cd84a4e6b89ba170a635987eb))

## [0.3.0](https://github.com/dignifiedquire/async-tar/compare/v0.2.0..v0.3.0) - 2020-07-29

### 🐛 Bug Fixes

- *(builder)* Add missing Send and Sync bounds to the builder - ([1d82b68](https://github.com/dignifiedquire/async-tar/commit/1d82b689bc00bcbb29c0bd7295730a044c7d6991))

## [0.2.0](https://github.com/dignifiedquire/async-tar/compare/v0.1.2..v0.2.0) - 2020-07-26

### ⛰️  Features

- Ensure Send and Sync for Archive, Entries - ([6e2533c](https://github.com/dignifiedquire/async-tar/commit/6e2533cc1d273d4fdf785cffc4d0f449ab80e4d4))

## [0.1.2](https://github.com/dignifiedquire/async-tar/compare/v0.1.1..v0.1.2) - 2020-07-21

### ⛰️  Features

- Add case for ustar header with gnu longname - ([085ff41](https://github.com/dignifiedquire/async-tar/commit/085ff4190e046fa2f9228bfc816b79dbe644e84c))

### 🐛 Bug Fixes

- Keep state accross yield points when reading data - ([2de9828](https://github.com/dignifiedquire/async-tar/commit/2de982823b93440f1d6527a94ac53c7f0d38bb3d))

## [0.1.1](https://github.com/dignifiedquire/async-tar/compare/v0.1.0..v0.1.1) - 2019-12-24

### 🐛 Bug Fixes

- Remove debug println - ([ff3a424](https://github.com/dignifiedquire/async-tar/commit/ff3a424575a2669bc0f3408060e75f668b26faca))

## [0.1.0] - 2019-12-24

### 📚 Documentation

- *(readme)* Updates - ([b72bb1b](https://github.com/dignifiedquire/async-tar/commit/b72bb1bd01b3cfbf38d1799938f1b30921b28189))
- Small fixes to the intro - ([f269f7b](https://github.com/dignifiedquire/async-tar/commit/f269f7b7f7968a0f44a410bbd705ba55f3631ff2))

### ⚙️ Miscellaneous Tasks

- Start testing for a minimum stable version - ([573ff28](https://github.com/dignifiedquire/async-tar/commit/573ff28195750790842a1f6442dc531414f55a17))
- Remove non existing feature - ([98712a4](https://github.com/dignifiedquire/async-tar/commit/98712a41fd84197cb65c1bfae0a571d7222dbf86))

### Builder

- Handle long link names - ([33a0b2f](https://github.com/dignifiedquire/async-tar/commit/33a0b2ffda6e8fc299d03ce67e66132414938ca0))
- Append_path_with_name() - ([181147a](https://github.com/dignifiedquire/async-tar/commit/181147ac738a4015d7e9dd436f60d44057d2ac48))

### Deps

- Update xattr to 0.2 - ([2fdfdff](https://github.com/dignifiedquire/async-tar/commit/2fdfdff070920cfc087e3dfb1624c199d1daab84))

### Entry

- Relax symlink target constraints - ([7843d6a](https://github.com/dignifiedquire/async-tar/commit/7843d6acfc9c9bc766b01cee86e6d2db37726c1f))
- Fix unpacking relative hardlinks - ([32bdb89](https://github.com/dignifiedquire/async-tar/commit/32bdb89155574a9229e7e3c4b376be0400891d25))

### Filetime

- Require 0.2.8 ([#214](https://github.com/dignifiedquire/async-tar/issues/214)) - ([8b141d0](https://github.com/dignifiedquire/async-tar/commit/8b141d0d7119a412373a558d0b884c519b6d44a8))


