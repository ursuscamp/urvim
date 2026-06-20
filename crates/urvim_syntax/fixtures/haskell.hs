-- Haskell syntax fixture
-- Multi-line comment
-- continues here
{-# LANGUAGE OverloadedStrings #-}
{-# OPTIONS_GHC -Wall #-}

module Greeter
  ( hello
  , answer
  , greet
  ) where

import Data.Text (Text, pack)
import qualified Data.Map as Map

value = "hello"
answer = 42
flag = True
falseFlag = False
letter = 'x'
floating = 3.14
neg = -42

greet :: Text -> Text
greet name = "hello, " <> name

factorial :: Integer -> Integer
factorial 0 = 1
factorial n = n * factorial (n - 1)

data Color = Red | Green | Blue deriving (Show, Eq)

data Tree = Leaf | Node Int Tree Tree

maybeVal :: Maybe Int
maybeVal = Just 42
noVal :: Maybe Int
noVal = Nothing

mapExample :: Map.Map String Int
mapExample = Map.fromList [("one", 1), ("two", 2)]

listExample :: [Int]
listExample = [1, 2, 3]

tupleExample :: (String, Int)
tupleExample = ("Ada", 42)

type Name = String
type Age = Int
type Person = (Name, Age)

demo :: IO ()
demo = do
  putStrLn "hello world"
  let x = 42
  print x
  mapM_ print [1, 2, 3]

ifExample :: Int -> String
ifExample n =
  if n > 0
    then "positive"
    else "non-positive"

caseExample :: Maybe Int -> String
caseExample mx =
  case mx of
    Just n  -> "got " ++ show n
    Nothing -> "got nothing"

whereExample :: Int -> Int
whereExample x = x + y + z
  where
    y = x * 2
    z = y + 1

lambdaExample :: Int -> Int
lambdaExample = \x -> x + 1
