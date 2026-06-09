/// 使用线性筛法（欧拉筛）返回 1 到 n 之间的所有素数。
///
/// # 参数
///
/// * `n` - 上界（包含），必须非负。若 `n < 2`，返回空向量。
///
/// # 返回值
///
/// 一个按升序排列的素数向量，类型为 `Vec<usize>`。
///
/// # 示例
///
/// ```
/// let primes = linear_sieve(30);
/// assert_eq!(primes, vec![2, 3, 5, 7, 11, 13, 17, 19, 23, 29]);
/// ```
pub fn linear_sieve(n: usize) -> Vec<usize> {
    if n < 2 {
        return Vec::new();
    }

    // 使用 bool 数组标记合数，下标对应整数
    let mut is_prime = vec![true; n + 1];
    is_prime[0] = false;
    is_prime[1] = false;

    let mut primes = Vec::new();

    // 线性筛的核心循环
    for i in 2..=n {
        if is_prime[i] {
            primes.push(i);
        }
        for &p in &primes {
            let composite = i * p;
            if composite > n {
                break;
            }
            is_prime[composite] = false;
            if i % p == 0 {
                break;
            }
        }
    }

    primes
}

fn main() {
    let primes = linear_sieve(30);
    assert_eq!(primes, vec![2, 3, 5, 7, 11, 13, 17, 19, 23, 29]);
}