(let x (block
	(let ret 0)
	(let cnt 0)
	(while (lt cnt 10)
		(set cnt (add cnt 1))
		(set ret (add cnt ret))
	)
))

(dump x) /* prints 55 */
