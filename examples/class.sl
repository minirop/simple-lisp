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

(let cat (new animal))
(print cat) /* prints "string: red" */
(set-colour cat "green")
(print cat) /* prints "string: green" */
(print "hello")
(print 42)
(print "==================")
(let t (new table))
(print t)
