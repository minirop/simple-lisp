(fun fib (x)
	(if (< x 3)
		(if (= x 0) 0 1)
		(+ (fib (- x 1)) (fib (- x 2)))
	)
)

(dump (fib 28))
