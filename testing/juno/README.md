# Juno Test Setup

The Juno Test setup is configured to run on the custom juno node but can work on the normal juno too.

The Reason for using the Custom node is to add the USDC which is not present on the regular node.



1. Setting up custom node setup
    
    a. Clone the Custome node repo

        git clone https://github.com/utkush/junoNodeCustomSetup
    
    b. Install Ignite to Compile the chain code 
   
        curl https://get.ignite.com/cli! | bash
    
    if there is an error using this command run it as 

        curl https://get.ignite.com/cli! | sudo bash

    c. Once Done 
        
        cd junoNodeCustomSetup

    d. Then once in the chain repo run 
        
        ignite chain serve 

    Doing this will start the chain now and post the memonics for alice and bob.
    
    Please copy either mneominic in presented and paste it as mnemonic in **testing/juno/wallet.js**
   
Once this is done we are ready to initate the tests.

Install the packages for Node js using yarn 

Make sure you're in **testing** dir and run 
        

        yarn 

Now run the tests using 
        

        node index.js