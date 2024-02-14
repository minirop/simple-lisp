(let guess -1)
(let answer (random 1 100))
(let count 0)

(while (neq guess answer)
	(print "Please input your guess: ")
	(set guess (read-int))
	(if (lt guess answer) (println "too small"))
	(if (gt guess answer) (println "too big"))
	(inc count)
)

(println "you won in " count " tries!!")
