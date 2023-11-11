(let x (block
	(let ret 0)
	(let cnt 0)
	(while (< cnt 10)
		(set cnt (+ cnt 1))
		(set ret (+ cnt ret))
	)
))

(dump x) /* prints 55 */
