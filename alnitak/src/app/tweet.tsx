interface TweetProps {
    message:string
}
const Tweet = (tweet: TweetProps) => {

    return (
        <div className="tweet">
            <br/>
            <strong>{ tweet.message ? tweet.message : "No message found" }</strong>
            <br/>Sent on noPhone.
        </div>
    )
}

export default Tweet
