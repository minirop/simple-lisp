(dump (add 1 2)) /* prints 3 */

(dump (sub 1 2)) /* prints -1 */

(fun test (a (b 0))
	(add (add a b) (sub 5 a))
)

(dump (test 5)) /* prints 5 */
(dump (test 6)) /* prints 5 */

(let c 0)
(fun test-two (a (b (add c c)))
	(add a b)
)

(dump (test-two 5))   /* prints 5 */
(dump (test-two 6 6)) /* prints 12 */

(fun lower (a b)
	(if (lt a b)
		"a is lower than b"
		"b is lower or equal than a"
	)
)

(dump (lower 1 2)) /* prints a is lower than b */
(dump (lower 2 1)) /* prints b is lower or equal than a */

(fun lower-or-equal (a b)
	(if (le a b)
		"a is lower or equal to b"
		"b is lower than a"
	)
)

(dump (lower-or-equal 1 2)) /* prints a is lower or equal to b */
(dump (lower-or-equal 2 1)) /* prints b is lower than a */

(fun test-return (x)
	(if (eq x 10)
		(return 1)
		(return 0)
	)
)

(dump (test-return 5))  /* prints 0 */
(dump (test-return 10)) /* prints 1 */

(fun test-return-string (x)
	(if (eq x "hello")
		(return "world")
		(return "nobody")
	)
)

(dump (test-return-string "hello")) /* prints world */
(dump (test-return-string "world")) /* prints nobody */
(dump (test-return-string 10))      /* prints nobody */

(fun fib (x)
	(if (lt x 3)
		(if (eq x 0) 0 1)
		(add (fib (sub x 1)) (fib (sub x 2)))
	)
)

(while (lt c 10)
	(set c (add c 1))
)

(set c 0)
(let sum-a (while (lt c 10)
	(set c (add c 1))
))
(dump sum-a) /* prints 10 */

(set c 0)
(let k 0)
(let sum-b (while (lt c 10)
	(set c (add c 1))
	(set k (add k c))
))
(dump sum-b) /* prints 55 */

(let x (list 1 2 3))
(dump x) /* prints something like [ int(1) int(1) int(1) ] (different between interpreted and compiled) */

(fun outer (x)
	(fun inner (y)
		(add y x)
	)

	(inner 3)
)

(dump (outer 45)) /* prints 48 */

(dump (inc 45)) /* prints 46 */
(dump (dec 45)) /* prints 44 */

(let xxx (call (fun (x)
		(add x x)
	)
	(call (fun () 4))
))
(dump xxx) /* prints 8 */

(let bg (fun (x) (div x 2)))
(dump bg) /* prints function: <lambda#1> */
(dump (bg 42)) /* prints 21 */

(fun pouet (l r)
	(if (gt l r) l r)
)
(dump (pouet 1 2)) /* prints 2 */
(dump (pouet 2 1)) /* prints 2 */

(let zero 0)
(fun zero () zero)
(dump zero)   /* prints 0 */
(dump (zero)) /* prints 0 */

(fun test-switch ((x (zero)))
	(switch x
		(case 0 "hello")
		(case 1 "world")
		"nobody"
	)
)
(dump (test-switch 0))      /* prints hello */
(dump (test-switch 1))      /* prints world */
(dump (test-switch 2))      /* prints nobody */
(dump (test-switch "test")) /* prints nobody */
(dump (fun () 0))           /* prints function: <lambda#1> */
(dump test-switch)          /* prints function: test-switch */
(dump eq)                    /* prints function: <native#1> */
