# Changelog

## [1.0.2](https://github.com/cam1se/stack-stat/compare/v1.0.1...v1.0.2) (2026-10-28)


### Bug Fixes

* **deps:** bump the stack-stat-prod group with 2 updates ([a6c487f](https://github.com/cam1se/stack-stat/commit/a6c487fc081f9741f4aad5de53788d53469cccfd))

## [1.0.1](https://github.com/cam1se/stack-stat/compare/v1.0.0...v1.0.1) (2026-10-21)


### Bug Fixes

* **providers:** ignore checks from cancelled runs a newer run replaced ([ebe8623](https://github.com/cam1se/stack-stat/commit/ebe8623c97e6316c346e3bc08792998429ee016a))

## [1.0.0](https://github.com/cam1se/stack-stat/compare/v0.9.0...v1.0.0) (2026-10-21)


### ⚠ BREAKING CHANGES

* **build:** stack-stat reaches 1.0.0. The keymap renamed in 0.9.0 (r refresh, d draft, t sort, R rebase) and the existing commands are now the stable public interface.

### Features

* **tui:** quit on escape ([f0c79dc](https://github.com/cam1se/stack-stat/commit/f0c79dcfba3722d750058d8c545af547f22e1bf1))
* **tui:** show command confirmation in a modal ([511e711](https://github.com/cam1se/stack-stat/commit/511e7114ecc9ed95905e0627706a758bcf460977)), closes [#57](https://github.com/cam1se/stack-stat/issues/57)


### Miscellaneous Chores

* **build:** release 1.0.0 ([12123db](https://github.com/cam1se/stack-stat/commit/12123db9fdfbfc0a28d5f779f6dbb77fb70abd59))

## [0.9.0](https://github.com/cam1se/stack-stat/compare/v0.8.1...v0.9.0) (2026-10-20)


### ⚠ BREAKING CHANGES

* **tui:** R no longer refreshes (use r), m no longer toggles draft (use d), and s no longer cycles sort (use t).

### Features

* **providers:** add stack rebase to the github provider ([9f32b73](https://github.com/cam1se/stack-stat/commit/9f32b73d54b9925b9b0fde89e004303e867ea41e))
* **tui:** add a rebase key and clean up the keymap ([35a9e3a](https://github.com/cam1se/stack-stat/commit/35a9e3ac582879d0a633905dadca5ce6826fd2e4))

## [0.8.1](https://github.com/cam1se/stack-stat/compare/v0.8.0...v0.8.1) (2026-10-19)


### Bug Fixes

* **update:** format sha2 0.11 digest as hex without LowerHex ([53deb51](https://github.com/cam1se/stack-stat/commit/53deb516f5ede9e8cb6a41a8bcc2470b60a49d62))

## [0.8.0](https://github.com/cam1se/stack-stat/compare/v0.7.0...v0.8.0) (2026-10-16)


### Features

* **tui:** lead rows, details, and next-to-merge with the PR title ([6bb6180](https://github.com/cam1se/stack-stat/commit/6bb61805acbcfafc4e2c820532de63e434a11d39)), closes [#49](https://github.com/cam1se/stack-stat/issues/49)

## [0.7.0](https://github.com/cam1se/stack-stat/compare/v0.6.0...v0.7.0) (2026-10-15)


### Features

* **core:** show unstacked prs and no-pr branches as a one-layer view ([298d871](https://github.com/cam1se/stack-stat/commit/298d871ad41f318b5d047d8af79f244bc74ef9b5)), closes [#26](https://github.com/cam1se/stack-stat/issues/26)


### Bug Fixes

* **core:** treat a closed unmerged pr as closed history ([b205aa3](https://github.com/cam1se/stack-stat/commit/b205aa3a209dc00a3a87bc75cbe508da19e8979d))
* **providers:** classify gh failures by observed output ([54fe616](https://github.com/cam1se/stack-stat/commit/54fe616a10adf258b0b8330ec4631ada599d4a08)), closes [#23](https://github.com/cam1se/stack-stat/issues/23)
* **providers:** lead failed git push errors with the rejection ([9d7c08d](https://github.com/cam1se/stack-stat/commit/9d7c08d8a030b036102c481278bba249d5d32d57))
* **providers:** show nothing to compare on the default branch ([59cd5ff](https://github.com/cam1se/stack-stat/commit/59cd5ff76ad437b2f2ed76aae7eb6fd0ac9aab6f))
* **tui:** name the actual trunk in the wide rebase label ([a2059c6](https://github.com/cam1se/stack-stat/commit/a2059c614adc79e72252d290e8e4bc56071ff40b))
* **tui:** refresh before mode-dependent actions after a branch switch ([cc75b35](https://github.com/cam1se/stack-stat/commit/cc75b359c6d452de8c00b07a5842e190ec6352ce))
* **tui:** remove the doubled blank line in no-pr details ([4b1d249](https://github.com/cam1se/stack-stat/commit/4b1d24980578198070bbc35febb4c8193a3c2d53))

## [0.6.0](https://github.com/cam1se/stack-stat/compare/v0.5.1...v0.6.0) (2026-10-13)


### Features

* **providers:** read stack structure from GitHub GraphQL ([dd20316](https://github.com/cam1se/stack-stat/commit/dd20316d68dbe235b5de5fd8748a8f67c90930cb)), closes [#16](https://github.com/cam1se/stack-stat/issues/16)
* **tui:** add leveled expiring notices and separate confirm prompt ([f0388e9](https://github.com/cam1se/stack-stat/commit/f0388e9c67d8a7dacdd994d32bdad2733818be7f)), closes [#16](https://github.com/cam1se/stack-stat/issues/16)
* **tui:** use GitHub stack discovery and link before actions ([4e5d7e3](https://github.com/cam1se/stack-stat/commit/4e5d7e335ea655f16e122dd200db805d904b5826)), closes [#16](https://github.com/cam1se/stack-stat/issues/16)


### Bug Fixes

* **providers:** ignore fork PRs when discovering the branch stack ([3de6a60](https://github.com/cam1se/stack-stat/commit/3de6a6020c961cbacdc19d90c18980c47b624e65)), closes [#16](https://github.com/cam1se/stack-stat/issues/16)

## [0.5.1](https://github.com/cam1se/stack-stat/compare/v0.5.0...v0.5.1) (2026-10-13)


### Bug Fixes

* **core:** derive rebase need from remote and mark local drift ([6866695](https://github.com/cam1se/stack-stat/commit/6866695b9d23b64c10a5c93d2658670657436acf))
* **tui:** move one layer per mouse wheel event ([5cff239](https://github.com/cam1se/stack-stat/commit/5cff2396039adc416b5cf4a6d955ec6f67c37fc4))

## [0.5.0](https://github.com/cam1se/stack-stat/compare/v0.4.0...v0.5.0) (2026-10-13)


### Features

* **tui:** block every layer above a blocking rebase ([083d77b](https://github.com/cam1se/stack-stat/commit/083d77ba41b45eb4160c8cf720350ab4017094a4))

## [0.4.0](https://github.com/cam1se/stack-stat/compare/v0.3.0...v0.4.0) (2026-10-13)


### Features

* **tui:** animate a startup wordmark until the first github result ([693cda8](https://github.com/cam1se/stack-stat/commit/693cda863f3108b8503cb3af482cb4a1aee2e6b4))


### Bug Fixes

* **tui:** select the worktree layer when github data arrives before git ([8ff5b62](https://github.com/cam1se/stack-stat/commit/8ff5b6206f6720998629ce0aa814e67bd2e1c9c6))

## [0.3.0](https://github.com/cam1se/stack-stat/compare/v0.2.0...v0.3.0) (2026-10-13)


### ⚠ BREAKING CHANGES

* **tui:** the j/k/g/G/J/K letter navigation keys are removed.

### Features

* **tui:** compact header identity with verdict pie chart ([f4d36dc](https://github.com/cam1se/stack-stat/commit/f4d36dc60f8f1ac3321434868a1a73db6962b270))
* **tui:** count unresolved review threads with optional gating ([5e58973](https://github.com/cam1se/stack-stat/commit/5e58973c29606394e9bc7b0b87a077f7a63f55d9))
* **tui:** detail scrollbar, passed before skipped, muted merged checks ([e2b8129](https://github.com/cam1se/stack-stat/commit/e2b8129a7ce37784ef554730c4a821b0d3420406))
* **tui:** dim merged rows entirely and show draft in red ([fcad91b](https://github.com/cam1se/stack-stat/commit/fcad91be012f32cdd8ee24806ee5f14b97c9ea51))
* **tui:** draw stack as a git-tree spine with verdict nodes ([34389f3](https://github.com/cam1se/stack-stat/commit/34389f3f7aac8ed4da522d1890959d2f0b5b851a))
* **tui:** go/no-go verdicts, always-open details, branding ([c318af0](https://github.com/cam1se/stack-stat/commit/c318af07e1a62b78dc830c3591856efe10092cbc))
* **tui:** green open state and accent-colored pane titles ([3f8de0e](https://github.com/cam1se/stack-stat/commit/3f8de0eb8fdba20d5b94a88d3ce064446ea8324e))
* **tui:** hide local column when no local checks exist ([893bc9c](https://github.com/cam1se/stack-stat/commit/893bc9c85598777de3c2ab38ee3bc25d898800c4))
* **tui:** larger verdict pie, next-to-merge and blocker rows in the header, wrapped detail header ([bc10e4c](https://github.com/cam1se/stack-stat/commit/bc10e4cfbbaf8dcfd8132c368ee56cdef9b83a89))
* **tui:** pulse marks the selected row, asterisk marks the worktree branch ([664ed04](https://github.com/cam1se/stack-stat/commit/664ed04ac32292a570d097c26f3831526485c3c5))
* **tui:** rebase column with optional trunk gating ([0a40d9c](https://github.com/cam1se/stack-stat/commit/0a40d9ca01b185f73f0895c8311c33d22914186c))
* **tui:** show a spinner in place of the checks icon while checks run ([98f4582](https://github.com/cam1se/stack-stat/commit/98f4582b42727f6464def6f8d3d891f83a852f21))
* **tui:** use a clock glyph for waiting states and logo blue and cyan accents ([ad23ce1](https://github.com/cam1se/stack-stat/commit/ad23ce1fbfe633bdca48ba76cbd899105187790b))
* **tui:** use one key set; arrows select, Page Up/Down scroll details, Home/End jump ([4271f75](https://github.com/cam1se/stack-stat/commit/4271f75bacd05f6759d62cee577d7736d678d2f0))
* **tui:** yellow review cell whenever a review is required ([4a768e9](https://github.com/cam1se/stack-stat/commit/4a768e901390063d9829efc007b4868fe01b4437))


### Bug Fixes

* **tui:** exclude skipped checks from the passed/total count ([cf32642](https://github.com/cam1se/stack-stat/commit/cf32642a250d0cc59847cf61864cb0fcffde70e0))
* **tui:** keep a colored spinner for running checks behind a failure ([2039462](https://github.com/cam1se/stack-stat/commit/203946238b595f1f3fe70ec039be20ecf9d5b594))

## [0.2.0](https://github.com/cam1se/stack-stat/compare/v0.1.0...v0.2.0) (2026-10-13)


### Features

* bootstrap stack-stat terminal dashboard ([f4e6bb3](https://github.com/cam1se/stack-stat/commit/f4e6bb3046d22ec6d5d3e8b516a5faf620b49937))

## Changelog

Release notes are generated by release-please from Conventional Commits.
