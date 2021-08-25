 # Iron Rose: (Invertable Bloom Filter for Rust)
 
 ## What is it?

 Iron Rose is a rust implementation of Invertable Bloom Filters as found in
 [What's the Difference? Efficient Set Reconciliation without Prior Context](https://www.ics.uci.edu/~eppstein/pubs/EppGooUye-SIGCOMM-11.pdf)

## Expected use
 ```rust
 use iron_rose::{Side, StrataEstimator, IBF};
 use uuid::Uuid;

 let mut estimator = StrataEstimator::default();
 let mut remote_estimator = StrataEstimator::default();
 let core = (0..1000)
     .map(|_| Uuid::new_v4().to_u128_le())
     .collect::<Vec<_>>();
 let local_ids = (0..50)
     .map(|_| Uuid::new_v4().to_u128_le())
     .collect::<Vec<_>>();
 let remote_ids = (0..50)
     .map(|_| Uuid::new_v4().to_u128_le())
     .collect::<Vec<_>>();
 let ids_from_database = core.iter().chain(local_ids.iter()).collect::<Vec<_>>();
 for x in ids_from_database.iter() {
     estimator.encode(**x);
 }
 for x in core.iter().chain(remote_ids.iter()) {
    remote_estimator.encode(*x);
 }
 // Retreive Remote Estimator in some way
 let ibf_size = estimator
     .estimate_differences(&remote_estimator)
     .expect("estimators should be same shape");
 let mut local = IBF::new(ibf_size);
 let mut remote = IBF::new(ibf_size);
 for x in ids_from_database.iter() {
     local.encode(**x);
 }
 for x in core.iter().chain(remote_ids.iter()) {
     remote.encode(*x);
 }
 // Retreive remote IBF
 let diff = (local - remote).expect("Local and remote should be the same shape");
 let differences = diff
     .decode()
     .expect("Successfully decoded because IBFs were large enough");
```

## Worthwhile Notes

Using Rust's trait system, we are actually able to say that anything that implements BitXOR and Serializable/Deserializable can be sent via an IBF, this means that we get the benifits of the IBF basic idea, but can encode larger and more complex things than just IDs.

Future enhancements "I plan to add"™ are wrappers around various basic types that allow us to send slices back and forth relativly easily.