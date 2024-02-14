(fun fib (x)
	(if (lt x 3)
		(if (eq x 0) 0 1)
		(add (fib (sub x 1)) (fib (sub x 2)))
	)
)

(dump (fib 28))
