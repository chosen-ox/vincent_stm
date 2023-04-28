use vincent_stm::atomically;
use vincent_stm::TVar;

fn main() {
    let tvar = TVar::new(5);
    let tvar1 = TVar::new(10);

    let res = atomically(|transaction| {
        tvar.write(transaction, 10).unwrap();
        assert_eq!(tvar.read(transaction).unwrap(), 10);

        tvar.write(transaction, 15).unwrap();
        assert_eq!(tvar.read(transaction).unwrap(), 15);

        tvar1.write(transaction, 20).unwrap();
        assert_eq!(transaction.read(&tvar1).unwrap(), 20);

        tvar.read(transaction)
    });

    assert_eq!(res, 15);
    println!("res: {}", res);

    let res1 = atomically(|trans| tvar1.read(trans));
    assert_eq!(res1, 20);
}
