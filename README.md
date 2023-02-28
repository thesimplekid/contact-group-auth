## Contact Group gRPC Server
[![License](https://img.shields.io/badge/License-BSD_3--Clause-blue.svg)](LICENSE)

# gRPC Extensions for nostr-rs-relay

gRPC authz server for [nostr-rs-rely](https://github.com/scsibug/nostr-rs-relay). Admits events based on the proximity of a social graph. Principal users are set in the config file, other users are categorized into tiers based on the proximity to that primary group.

If the relay has nip42 enabled it will use the authenticated pubkey if not the author pubkey of the note will be used. 

# Tiers

- Principal users 
- Primary follows - users that are followed by principal follows 
- Secondary follows - users that are followed by primary follows
- Tertiary follows - users that are followed by secondary follows 
- Others

Ability to enable/disable posts and rate limits can be defined for each tier.

Currently only ability to post, posts per hour and posts per day are the limitation but more will be added for example max event size, kind, tag content etc. 

## License

Code is under the [BSD 3-Clause License](LICENSE-BSD-3)

## Contribution

All contributions welcome.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, shall be licensed as above, without any additional terms or conditions.

## Contact

I can be contacted for comments or questions on nostr at _@thesimplekid.com (npub1qjgcmlpkeyl8mdkvp4s0xls4ytcux6my606tgfx9xttut907h0zs76lgjw) or via email tsk@thesimplekid.com.

## TODO:
- [ ] async
- [ ] DB will often have to be deleted and recreated fairly often to fix some corruption error.  Seems to function okay but if database is closed cant be reopened.
- [ ] db entries should probably be trimmed 