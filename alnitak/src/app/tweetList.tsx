"use client";

import { useState, useEffect } from "react"
import Tweet from "./tweet"

interface Tweets {
    id: number
    message: string
}
interface tweetsAPIResult {
    results: Tweets[]
}

const TweetList = () => {
    const [tweets, setTweets] = useState<Tweets[]>([])
    useEffect(() => {
        async function getData() {
            const settings: RequestInit = {
                method: 'GET',
                mode: 'no-cors',
                headers: {
                    'Content-Type': 'application/json',
                    'Accept': 'application/json',
                }

            }
            const response = await fetch("/tweets", settings)
            const res: tweetsAPIResult = await response.json()
            // console.log('Await json: ', await response.json())
            setTweets(res.results)
        }
        getData()
    }, []);
    // console.log(tweets)
    return (
        <div>
            { tweets.map((twt) => {
                return <Tweet key={twt.id} message={twt.message} />
                })
            }
        </div>
    )
}


export default TweetList
