# License decision record

- **Decision date:** 2026-07-26
- **Copyright holder:** Sealdot404
- **Decision:** License the ClipRiva client repository under Apache License 2.0.
- **Scope:** Source code and documentation in this repository. The ClipRiva name and logos are
  governed separately by [TRADEMARK.md](../../TRADEMARK.md).

This record is an engineering and product decision aid, not legal advice. Consult a qualified lawyer
before changing the license, accepting a material third-party code grant, forming an entity, or
registering a mark.

## Options considered

| Dimension | Apache-2.0 | MIT | GPLv3 | AGPLv3 | Dual license | Open Core |
| --- | --- | --- | --- | --- | --- |
| Commercial use | Allowed under license conditions | Allowed under license conditions | Allowed under license conditions | Allowed under license conditions | Depends on selected terms | Depends on the open component license |
| Closed-source derivative distribution | Allowed if notices and conditions are retained | Allowed if notice is retained | Not allowed for a conveyed combined derivative without GPLv3 source obligations | Not allowed for a conveyed combined derivative without AGPLv3 obligations | Depends on which licensee path applies | Depends on the boundary between open and proprietary components |
| Express patent grant | Yes, with termination condition | No express patent clause in the text | Yes, subject to GPLv3 terms | Yes, subject to AGPLv3 terms | Depends on selected licenses | Depends on the chosen open-source license |
| Network-service source obligation | No | No | No additional network-use trigger | Yes, for modified network-interactive versions | Depends on selected licenses | Depends on the chosen open-source license |
| Community expectation | Permissive, contribution-friendly | Very simple permissive choice | Strong reciprocal copyleft | Strong network copyleft | More operational and legal complexity | A distribution model, not a license |
| Effect on ClipRiva | Fits an offline client Core and keeps future official services separate | Similar permissive outcome, without Apache's explicit patent terms | Would require a different strategy for downstream derivatives | Would impose a network-source condition that does not match the current local-only Core | Not needed for the current single-repository scope | Not needed while official service code is absent |

## Selected approach

ClipRiva uses Apache-2.0 for this repository because the project owner selected a permissive client
license with an explicit patent grant. The `LICENSE` file names Sealdot404 in the Apache appendix.
Contributions submitted through the repository are handled under the existing Apache-2.0 contribution
clause unless a separate written agreement states otherwise.

Future official sync, identity, billing, administration, monitoring, signing credentials, and
production deployment configuration are outside this repository and are not licensed by this
decision. This is a scope boundary, not a promise that such services will exist.

## Implementation checklist

- [x] Keep the canonical Apache-2.0 text in `LICENSE`.
- [x] Record the public copyright holder as Sealdot404.
- [x] Keep third-party dependencies and notices in `THIRD_PARTY_NOTICES.md` and
  `docs/audit/dependency-inventory.md`.
- [x] Keep name/logo permission separate in `TRADEMARK.md`.
- [ ] Generate and review a full transitive SBOM/license report before the first public release.
- [ ] Review any third-party code, asset, font, media, or contribution with unclear provenance
  before accepting it.
- [ ] Obtain professional advice before any relicensing, dual-licensing, or trademark registration.

## Reference materials

The Apache-2.0 text includes an explicit patent grant and a separate trademark clause. Its official
application guidance recommends a `LICENSE` file and considers a `NOTICE` file where appropriate.
See [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0) and
[Apache's application guidance](https://www.apache.org/legal/apply-license). The GPL/AGPL and MIT
columns summarize their respective license texts for product comparison only; they do not replace a
legal review.
