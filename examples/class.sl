(class animal
	(let colour "red")
	(fun print ()
		(dump colour)
	)
	(fun set-colour (c)
		(set colour c)
	)
)

(class table
	(let cat (new animal))
	(fun print ()
		(print cat)
	)
)

(fun print (c)
	(dump c)
)

(class cat animal
	(let pawn 4)
	(fun get-colour ()
		colour
	)
)

(class persian cat)

(let kitty (new persian))
(print kitty) /* prints "string: red" */
(dump (get-colour kitty)) /* prints "string: red" */
(set-colour kitty "green")
(print kitty) /* prints "string: green" */
(dump (get-colour kitty)) /* prints "string: green" */
