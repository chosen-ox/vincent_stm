# vincent_stm
Implement software transactional memory in Rust. The implementation aims to speedup the original STM. Here's some possible speedup methods:

- larger log commit are always canceled by small log commit which is waste of time.

Solution1: For large log commit, if its number of retry exceeds some limit, it will be committed forcely by holding the lock.

Solution2: If only few read_TVar is modified, we can recalculate the possible write_TVar and commit instead of retrying later.

- Transaction cannot contain operation which has side effect(In other words, you can’t execute operations in a transaction, the action of which can’t be cancelled )

Solution: We execute those operation only when verification is successful.

- When a transaction wait for a resource ready, it's time-consuming(e.g. wait for a empty queue to be enqueued)

Solution: Use cond_variable to notify the transaction.

# Evaluation:

Compare my implementation with both current rust stm and C++ stm to obtain possible speedup.


# Ref:

Nir Shavit and Dan Touitou. Software transactional memory. Distributed Computing. Volume 10, Number 2. February 1997.

H. Alan Beadle, Wentao Cai, Haosen Wen, and Michael L. Scott. 2020. Nonblocking persistent software transactional memory. In Proceedings of the 25th ACM SIGPLAN Symposium on Principles and Practice of Parallel Programming (PPoPP '20). Association for Computing Machinery, New York, NY, USA, 429–430. https://doi.org/10.1145/3332466.3374506

C++ STM lib: https://github.com/mfs409/rstm

Rust STM: https://github.com/Marthog/rust-stm

Hashkell STM Implementation Guide: https://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.365.1337&rep=rep1&type=pdf
