use vincent_stm::atomically;
use vincent_stm::Tvar;

fn main() {
    let tvar = Tvar::new(5);
    // let space = Space::new(1);
    // let tvar1 = Tvar::new_with_space(10, space);
    let tvar1 = Tvar::new(10);
    let res = atomically(|transaction| {
        tvar.write(10, transaction)?;
        assert_eq!(tvar.read(transaction).unwrap(), 10);
        tvar.write(15, transaction)?;
        assert_eq!(tvar.read(transaction).unwrap(), 15);
        tvar1.write(20, transaction)?;
        assert_eq!(transaction.read(&tvar1).unwrap(), 20);
        tvar.read(transaction)
    });
    assert_eq!(res, 15);
    println!("res: {}", res);
    let res1 = atomically(|trans| tvar1.read(trans));
    assert_eq!(res1, 20);
}
